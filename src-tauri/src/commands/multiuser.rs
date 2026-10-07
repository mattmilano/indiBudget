//! Hosting and connecting, driven from the local screens.
//!
//! None of these are registered in the boundary registry: they are about *this*
//! machine — whether it is hosting, which host it joined, what it is connected
//! to — and are meaningless or dangerous asked remotely.
//!
//! # Every command here runs off the main thread
//!
//! Tauri runs a command that is not `async` on the app's main thread. These
//! commands wait on the network, and a network can take a long time to say no:
//! a firewall that silently drops traffic leaves a connection attempt waiting
//! for a minute or more. On the main thread that wait froze the whole window,
//! needing a force-quit. So each command is `async` and does its work through
//! [`off_main_thread`], and the socket itself has timeouts (see `net::client`),
//! so the wait is both bounded and invisible to the window.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

use super::AppState;
use crate::boundary::registry::{dispatch, BoundaryCtx};
use crate::boundary::{Actor, BoundaryError, Request, Response};
use crate::database::repository;
use crate::net::addresses::{parse_host_address, reachable_addresses, DEFAULT_PORT};
use crate::net::client::Client;
use crate::net::credentials;
use crate::net::discovery::{self, Advertisement, FoundHost};
use crate::net::host::{self, HostState, RunningHost, Seat};
use crate::net::identity::{Fingerprint, HostIdentity};

/// The settings key under which a joining computer remembers its host.
const SAVED_HOST_KEY: &str = "joined_host";

/// This machine's part in a shared budget.
#[derive(Default)]
pub struct MultiUser {
    running: Mutex<Option<RunningHost>>,
    state: Mutex<Option<Arc<HostState>>>,
    client: Mutex<Option<Client>>,
    connected_as: Mutex<Option<String>>,
    /// Set when the connection to a host failed partway through.
    ///
    /// While set, requests for budget data are refused rather than answered
    /// from this computer's own database. Answering locally would quietly show
    /// a different — usually empty — budget, and look like everything had been
    /// lost.
    lost: Mutex<Option<String>>,
    /// The announcement that lets joining computers find this one, while
    /// hosting. Absent when the network would not carry it.
    advert: Mutex<Option<Advertisement>>,
}

impl MultiUser {
    pub fn new() -> Self {
        Self::default()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn db_of(state: &AppState) -> Result<Arc<crate::database::Database>, String> {
    let guard = lock(&state.db);
    Ok(Arc::clone(guard.as_ref().ok_or("Database not initialized")?))
}

/// Run `work` on a background thread with the app's state.
async fn off_main_thread<T, F>(app: AppHandle, work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || work(&app.state::<AppState>()))
        .await
        .map_err(|e| format!("indiBudget hit an internal error: {e}"))?
}

// ------------------------------------------------------------ remembering

/// What a joining computer remembers about the host it paired with.
///
/// Stored in this computer's own database, so a restart needs only a sign-in
/// rather than a fresh pairing. The device token is the credential that says
/// "this machine was deliberately added"; it never goes to the frontend, and
/// it lives in the operating system's keychain wherever there is one (see
/// `net::credentials`). The password is never stored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedHost {
    pub address: String,
    pub fingerprint: String,
    /// Only when this computer has no keychain to keep it in. Hosts saved
    /// before the keychain was used have it here too, and it moves to the
    /// keychain at their next sign-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_token: Option<String>,
    pub label: String,
    pub last_login: Option<String>,
}

impl SavedHost {
    /// The keychain entry for this host's token. Named by the host's identity,
    /// which is what the token belongs to.
    fn keychain_account(&self) -> String {
        format!("host-{}", self.fingerprint)
    }

    fn device_token(&self) -> Option<String> {
        self.device_token
            .clone()
            .or_else(|| credentials::load(&self.keychain_account()))
    }

    /// Keep a token, in the keychain if this computer has one.
    fn keep_token(&mut self, token: String) {
        self.device_token = if credentials::store(&self.keychain_account(), &token) {
            None
        } else {
            Some(token)
        };
    }

    fn forget_token(&self) {
        credentials::forget(&self.keychain_account());
    }
}

/// The part of [`SavedHost`] the screens may see.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SavedHostView {
    pub address: String,
    pub fingerprint_groups: String,
    pub label: String,
    pub last_login: Option<String>,
}

fn load_saved_host(state: &AppState) -> Result<Option<SavedHost>, String> {
    let Ok(db) = db_of(state) else {
        return Ok(None);
    };
    let raw = db
        .with_connection(|conn| repository::get_setting(conn, SAVED_HOST_KEY))
        .map_err(|e| e.to_string())?;
    Ok(raw.and_then(|r| serde_json::from_str(&r).ok()))
}

fn store_saved_host(state: &AppState, saved: &SavedHost) -> Result<(), String> {
    let db = db_of(state)?;
    let raw = serde_json::to_string(saved).map_err(|e| e.to_string())?;
    db.with_connection(|conn| repository::set_setting(conn, SAVED_HOST_KEY, &raw))
        .map_err(|e| e.to_string())
}

fn clear_saved_host(state: &AppState) -> Result<(), String> {
    let db = db_of(state)?;
    db.with_connection(|conn| repository::delete_setting(conn, SAVED_HOST_KEY))
        .map_err(|e| e.to_string())
}

// ----------------------------------------------------------------- status

#[derive(Debug, Serialize)]
pub struct HostingStatus {
    pub hosting: bool,
    /// The first of `addresses`, for places that show only one.
    pub address: Option<String>,
    /// Where other computers can reach this one. Never the `0.0.0.0` the
    /// listener binds to, which a joining computer would read as itself.
    pub addresses: Vec<String>,
    pub fingerprint: Option<String>,
    pub fingerprint_groups: Option<String>,
    pub pairing: bool,
    pub connected: bool,
    pub signed_in_as: Option<String>,
    /// The host this computer joined, if it has paired with one.
    pub saved_host: Option<SavedHostView>,
    /// The connection to the host dropped and has not been re-established.
    pub lost: bool,
    /// Why, in a sentence: the host went away, or ended this person's session.
    pub lost_reason: Option<String>,
    /// While hosting: who is signed in from other computers right now.
    pub connected_people: Vec<Seat>,
}

pub fn status_of(state: &AppState) -> HostingStatus {
    let (hosting, addresses) = {
        let running = lock(&state.multi_user.running);
        let addresses = running
            .as_ref()
            .map(|r| reachable_addresses(r.addr().port()))
            .unwrap_or_default();
        (running.is_some(), addresses)
    };
    let (fingerprint, pairing, connected_people) = {
        let host_state = lock(&state.multi_user.state);
        (
            host_state.as_ref().map(|s| s.identity.fingerprint()),
            host_state.as_ref().map(|s| s.is_pairing()).unwrap_or(false),
            host_state.as_ref().map(|s| s.connected()).unwrap_or_default(),
        )
    };
    let saved_host = load_saved_host(state).ok().flatten().map(|s| SavedHostView {
        fingerprint_groups: Fingerprint::from_hex(&s.fingerprint)
            .map(|f| f.display_groups())
            .unwrap_or_default(),
        address: s.address,
        label: s.label,
        last_login: s.last_login,
    });

    // Each lock is taken and released in its own statement. Inside one struct
    // literal the guards are temporaries that all live until the literal ends,
    // so taking the same mutex twice there deadlocks the thread — which is
    // exactly what reading `lost` twice did.
    let connected = lock(&state.multi_user.client).is_some();
    let signed_in_as = lock(&state.multi_user.connected_as).clone();
    let lost_reason = lock(&state.multi_user.lost).clone();

    HostingStatus {
        hosting,
        address: addresses.first().cloned(),
        addresses,
        fingerprint: fingerprint.map(|f| f.to_hex()),
        fingerprint_groups: fingerprint.map(|f| f.display_groups()),
        pairing,
        connected,
        signed_in_as,
        saved_host,
        lost: lost_reason.is_some(),
        lost_reason,
        connected_people,
    }
}

#[tauri::command]
pub async fn hosting_status(app: AppHandle) -> Result<HostingStatus, String> {
    off_main_thread(app, |state| Ok(status_of(state))).await
}

// ---------------------------------------------------------------- hosting

pub fn start_hosting_on(state: &AppState, port: Option<u16>) -> Result<HostingStatus, String> {
    if lock(&state.multi_user.running).is_some() {
        return Err("This computer is already hosting.".into());
    }
    if lock(&state.multi_user.client).is_some() {
        return Err("This computer is connected to someone else's budget. \
                    Disconnect before hosting your own."
            .into());
    }

    let port = port.unwrap_or(DEFAULT_PORT);
    if port == 0 {
        return Err("Choose a port between 1 and 65535, or leave it blank for the default.".into());
    }

    let db = db_of(state)?;
    let identity = db
        .with_connection(|conn| Ok(HostIdentity::load_or_create(conn)))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.sentence())?;

    // The same shared state the local session uses, so holds and news are one
    // set rather than two.
    let host_state = Arc::new(HostState::new(
        db,
        Arc::clone(&state.registry),
        identity,
        Arc::clone(&state.shared),
    ));

    let running = host::start(Arc::clone(&host_state), SocketAddr::from(([0, 0, 0, 0], port)))
        .map_err(|e| e.sentence())?;

    // Best effort: a network that blocks multicast still lets people type the
    // address shown on the Sharing screen.
    let advert = Advertisement::start(running.addr().port(), &host_state.identity.fingerprint()).ok();

    *lock(&state.multi_user.running) = Some(running);
    *lock(&state.multi_user.state) = Some(host_state);
    *lock(&state.multi_user.advert) = advert;
    Ok(status_of(state))
}

#[tauri::command]
pub async fn start_hosting(app: AppHandle, port: Option<u16>) -> Result<HostingStatus, String> {
    off_main_thread(app, move |state| start_hosting_on(state, port)).await
}

pub fn stop_hosting_on(state: &AppState) -> HostingStatus {
    // Withdrawn first, so nobody picks this computer from a list as it stops.
    let advert = lock(&state.multi_user.advert).take();
    drop(advert);
    let running = lock(&state.multi_user.running).take();
    if let Some(mut running) = running {
        // Also disconnects every computer that had joined.
        running.stop();
    }
    *lock(&state.multi_user.state) = None;
    status_of(state)
}

#[tauri::command]
pub async fn stop_hosting(app: AppHandle) -> Result<HostingStatus, String> {
    off_main_thread(app, |state| Ok(stop_hosting_on(state))).await
}

pub fn open_pairing_on(state: &AppState) -> Result<String, String> {
    let host_state = lock(&state.multi_user.state);
    Ok(host_state
        .as_ref()
        .ok_or("Start hosting before pairing another computer.")?
        .open_pairing())
}

#[tauri::command]
pub async fn open_pairing(app: AppHandle) -> Result<String, String> {
    off_main_thread(app, open_pairing_on).await
}

#[tauri::command]
pub async fn close_pairing(app: AppHandle) -> Result<(), String> {
    off_main_thread(app, |state| {
        if let Some(host_state) = lock(&state.multi_user.state).as_ref() {
            host_state.close_pairing();
        }
        Ok(())
    })
    .await
}

// ---------------------------------------------------------------- joining

/// Listen briefly for computers hosting a budget on this network.
#[tauri::command]
pub async fn discover_hosts(app: AppHandle) -> Result<Vec<FoundHost>, String> {
    off_main_thread(app, |_| {
        discovery::browse(discovery::LISTEN_FOR).map_err(|e| {
            format!("Could not look for computers on this network ({e}). Type the host's address instead.")
        })
    })
    .await
}

#[derive(Debug, Deserialize)]
pub struct PairRequest {
    pub address: String,
    pub code: String,
    pub label: String,
}

/// Pair with a host and remember it.
///
/// Saved the moment pairing succeeds rather than after sign-in: the host keeps
/// only the token's hash, so a token not saved here is gone for good, leaving
/// an orphaned entry on the host and a computer that has to pair again.
pub fn pair_on(state: &AppState, request: PairRequest) -> Result<HostingStatus, String> {
    if lock(&state.multi_user.running).is_some() {
        return Err("This computer is hosting its own budget. \
                    Stop hosting before joining another."
            .into());
    }
    let addr = parse_host_address(&request.address)?;

    let mut client = Client::connect_for_pairing(addr).map_err(|e| e.sentence())?;
    let fingerprint = client
        .host_fingerprint()
        .ok_or("That computer did not present an identity.")?;

    // Pairing again with the same host hands back the old token, so the host
    // swaps that entry instead of listing this computer twice. Only once the
    // host has shown the identity remembered from last time: this connection
    // is not pinned, and the old token must not go to anyone else.
    let earlier = load_saved_host(state)?;
    let same_host = earlier
        .as_ref()
        .filter(|e| e.fingerprint == fingerprint.to_hex());
    let replaces = same_host.and_then(SavedHost::device_token);

    let token = client
        .pair_again(&request.code, &request.label, replaces.as_deref())
        .map_err(|e| e.sentence())?;

    if let Some(earlier) = earlier.as_ref().filter(|e| e.fingerprint != fingerprint.to_hex()) {
        earlier.forget_token();
    }
    let mut saved = SavedHost {
        address: addr.to_string(),
        fingerprint: fingerprint.to_hex(),
        device_token: None,
        label: request.label.trim().to_string(),
        last_login: same_host.and_then(|e| e.last_login.clone()),
    };
    saved.keep_token(token);
    store_saved_host(state, &saved)?;
    Ok(status_of(state))
}

#[tauri::command]
pub async fn pair_with_host(app: AppHandle, request: PairRequest) -> Result<HostingStatus, String> {
    off_main_thread(app, move |state| pair_on(state, request)).await
}

#[derive(Debug, Deserialize)]
pub struct ConnectRequest {
    pub login: String,
    pub password: String,
    /// A new address for the remembered host, when its address on the network
    /// changed. The identity it was paired with still has to match, so this
    /// cannot be used to reach a different computer.
    #[serde(default)]
    pub address: Option<String>,
}

/// Sign in to the remembered host.
pub fn connect_on(state: &AppState, request: ConnectRequest) -> Result<HostingStatus, String> {
    if lock(&state.multi_user.running).is_some() {
        return Err("This computer is hosting its own budget. \
                    Stop hosting before connecting to another."
            .into());
    }
    let mut saved = load_saved_host(state)?
        .ok_or("This computer has not been paired with a host yet. Pair it first.")?;

    if let Some(address) = request.address.as_deref().filter(|a| !a.trim().is_empty()) {
        saved.address = parse_host_address(address)?.to_string();
    }
    let addr = parse_host_address(&saved.address)?;
    let fingerprint = Fingerprint::from_hex(&saved.fingerprint).map_err(|e| e.sentence())?;

    let token = saved.device_token().ok_or(
        "This computer's pairing with that host could not be found in the system keychain. \
         Unlock the keychain and try again, or forget the host and pair again.",
    )?;

    let mut client = Client::connect(addr, fingerprint).map_err(|e| e.sentence())?;
    let session = client
        .sign_in(&token, &request.login, &request.password)
        .map_err(|e| e.sentence())?;

    saved.last_login = Some(request.login.trim().to_string());
    // A host remembered before the keychain was used moves its token there now.
    if saved.device_token.is_some() {
        saved.keep_token(token);
    }
    store_saved_host(state, &saved)?;

    *lock(&state.multi_user.client) = Some(client);
    *lock(&state.multi_user.connected_as) = Some(session.display_name);
    *lock(&state.multi_user.lost) = None;
    Ok(status_of(state))
}

#[tauri::command]
pub async fn connect_to_host(
    app: AppHandle,
    request: ConnectRequest,
) -> Result<HostingStatus, String> {
    off_main_thread(app, move |state| connect_on(state, request)).await
}

pub fn disconnect_on(state: &AppState) -> HostingStatus {
    *lock(&state.multi_user.client) = None;
    *lock(&state.multi_user.connected_as) = None;
    *lock(&state.multi_user.lost) = None;
    status_of(state)
}

#[tauri::command]
pub async fn disconnect_from_host(app: AppHandle) -> Result<HostingStatus, String> {
    off_main_thread(app, |state| Ok(disconnect_on(state))).await
}

/// Disconnect and forget the remembered host. Joining again needs a fresh
/// pairing. The host still lists this computer until someone revokes it there.
pub fn forget_on(state: &AppState) -> Result<HostingStatus, String> {
    disconnect_on(state);
    if let Some(saved) = load_saved_host(state)? {
        saved.forget_token();
    }
    clear_saved_host(state)?;
    Ok(status_of(state))
}

#[tauri::command]
pub async fn forget_saved_host(app: AppHandle) -> Result<HostingStatus, String> {
    off_main_thread(app, forget_on).await
}

// -------------------------------------------------------------- the door

/// Run a boundary command against whatever this machine is attached to.
///
/// Connected to someone else's budget, it goes over the wire. Otherwise it is
/// dispatched locally against the same registry, the same holds and the same
/// news — which is what makes a hold taken here block a laptop, and a change
/// made on a laptop show up here.
#[tauri::command]
pub async fn boundary_invoke(
    app: AppHandle,
    command: String,
    args: serde_json::Value,
) -> Result<Response, String> {
    off_main_thread(app, move |state| invoke_on(state, command, args)).await
}

/// The body of `boundary_invoke`, separated from Tauri so the real startup and
/// joining sequences can be exercised in tests.
pub fn invoke_on(
    state: &AppState,
    command: String,
    args: serde_json::Value,
) -> Result<Response, String> {
    let request = Request::new(command, args);

    // Answer "not registered" before anything else. Host-only commands are
    // dispatched directly by the frontend once told they are not registered —
    // and `init_app`, the command that *opens* the database, is one of them.
    // Requiring an open database first meant the database could never be
    // opened. The registry is identical on every computer, so this answer is
    // the same one a host would give.
    if !state.registry.contains(&request.command) {
        return Ok(Response::err(BoundaryError::UnknownCommand {
            command: request.command,
        }));
    }

    {
        let mut slot = lock(&state.multi_user.client);
        if let Some(client) = slot.as_mut() {
            let result = client.invoke(request);
            if client.is_broken() {
                // Either a late reply could now arrive as the answer to the
                // next request, or the host ended the session. Either way this
                // connection is finished.
                *slot = None;
                drop(slot);
                *lock(&state.multi_user.connected_as) = None;
                let reason = result
                    .as_ref()
                    .err()
                    .map(|e| e.sentence())
                    .unwrap_or_else(|| "The connection to the host was lost.".into());
                *lock(&state.multi_user.lost) = Some(reason);
            }
            return result.map_err(|e| e.sentence());
        }
    }

    // Bound first: an `if let` keeps its scrutinee's lock guard alive for the
    // whole block, which is one future edit away from the double-lock above.
    let lost_reason = lock(&state.multi_user.lost).clone();
    if let Some(reason) = lost_reason {
        return Err(format!("{reason} Nothing here can be shown until you sign in again from Sharing."));
    }

    let db = db_of(state)?;
    let actor = Actor::local_owner();
    let ctx = BoundaryCtx::new(&db, &actor, &state.shared);
    Ok(dispatch(&state.registry, &ctx, request))
}
