//! Hosting and connecting, driven from the local screens.
//!
//! None of these are registered in the boundary registry: they are about *this*
//! machine — whether it is hosting, which machines it paired, what it is
//! connected to — and are meaningless or dangerous asked remotely.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tauri::State;

use super::AppState;
use crate::boundary::registry::{dispatch, BoundaryCtx};
use crate::boundary::{Actor, Request, Response};
use crate::net::client::Client;
use crate::net::host::{self, HostState, RunningHost};
use crate::net::identity::{Fingerprint, HostIdentity};

/// This machine's part in a shared budget.
#[derive(Default)]
pub struct MultiUser {
    running: Mutex<Option<RunningHost>>,
    state: Mutex<Option<Arc<HostState>>>,
    client: Mutex<Option<Client>>,
    connected_as: Mutex<Option<String>>,
}

impl MultiUser {
    pub fn new() -> Self {
        Self::default()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn db_of(state: &State<AppState>) -> Result<Arc<crate::database::Database>, String> {
    let guard = lock(&state.db);
    Ok(Arc::clone(guard.as_ref().ok_or("Database not initialized")?))
}

#[derive(Debug, Serialize)]
pub struct HostingStatus {
    pub hosting: bool,
    pub address: Option<String>,
    pub fingerprint: Option<String>,
    pub fingerprint_groups: Option<String>,
    pub pairing: bool,
    pub connected: bool,
    pub signed_in_as: Option<String>,
}

fn status_of(state: &State<AppState>) -> HostingStatus {
    let running = lock(&state.multi_user.running);
    let host_state = lock(&state.multi_user.state);
    let fingerprint = host_state.as_ref().map(|s| s.identity.fingerprint());

    HostingStatus {
        hosting: running.is_some(),
        address: running.as_ref().map(|r| r.addr().to_string()),
        fingerprint: fingerprint.map(|f| f.to_hex()),
        fingerprint_groups: fingerprint.map(|f| f.display_groups()),
        pairing: host_state.as_ref().map(|s| s.is_pairing()).unwrap_or(false),
        connected: lock(&state.multi_user.client).is_some(),
        signed_in_as: lock(&state.multi_user.connected_as).clone(),
    }
}

#[tauri::command]
pub fn hosting_status(state: State<AppState>) -> HostingStatus {
    status_of(&state)
}

#[tauri::command]
pub fn start_hosting(state: State<AppState>, port: Option<u16>) -> Result<HostingStatus, String> {
    if lock(&state.multi_user.running).is_some() {
        return Err("This computer is already hosting.".into());
    }
    if lock(&state.multi_user.client).is_some() {
        return Err("This computer is connected to someone else's budget. \
                    Disconnect before hosting your own."
            .into());
    }

    let db = db_of(&state)?;
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

    let addr: SocketAddr = format!("0.0.0.0:{}", port.unwrap_or(0))
        .parse()
        .map_err(|_| "That is not a usable port.".to_string())?;
    let running = host::start(Arc::clone(&host_state), addr).map_err(|e| e.sentence())?;

    *lock(&state.multi_user.running) = Some(running);
    *lock(&state.multi_user.state) = Some(host_state);
    Ok(status_of(&state))
}

#[tauri::command]
pub fn stop_hosting(state: State<AppState>) -> HostingStatus {
    if let Some(mut running) = lock(&state.multi_user.running).take() {
        running.stop();
    }
    *lock(&state.multi_user.state) = None;
    status_of(&state)
}

#[tauri::command]
pub fn open_pairing(state: State<AppState>) -> Result<String, String> {
    let host_state = lock(&state.multi_user.state);
    Ok(host_state
        .as_ref()
        .ok_or("Start hosting before pairing another computer.")?
        .open_pairing())
}

#[tauri::command]
pub fn close_pairing(state: State<AppState>) {
    if let Some(host_state) = lock(&state.multi_user.state).as_ref() {
        host_state.close_pairing();
    }
}

// ------------------------------------------------------------ connecting

#[derive(Debug, Deserialize)]
pub struct PairRequest {
    pub address: String,
    pub code: String,
    pub label: String,
}

#[derive(Debug, Serialize)]
pub struct PairedHost {
    pub device_token: String,
    pub fingerprint: String,
    pub fingerprint_groups: String,
}

#[tauri::command]
pub fn pair_with_host(request: PairRequest) -> Result<PairedHost, String> {
    let addr: SocketAddr = request.address.parse().map_err(|_| {
        "That does not look like an address. It should be like 192.168.1.20:7420".to_string()
    })?;

    let mut client = Client::connect_for_pairing(addr).map_err(|e| e.sentence())?;
    let fingerprint = client
        .host_fingerprint()
        .ok_or("That computer did not present an identity.")?;
    let token = client
        .pair(&request.code, &request.label)
        .map_err(|e| e.sentence())?;

    Ok(PairedHost {
        device_token: token,
        fingerprint: fingerprint.to_hex(),
        fingerprint_groups: fingerprint.display_groups(),
    })
}

#[derive(Debug, Deserialize)]
pub struct ConnectRequest {
    pub address: String,
    pub fingerprint: String,
    pub device_token: String,
    pub login: String,
    pub password: String,
}

#[tauri::command]
pub fn connect_to_host(
    state: State<AppState>,
    request: ConnectRequest,
) -> Result<HostingStatus, String> {
    if lock(&state.multi_user.running).is_some() {
        return Err("This computer is hosting its own budget. \
                    Stop hosting before connecting to another."
            .into());
    }

    let addr: SocketAddr = request
        .address
        .parse()
        .map_err(|_| "That does not look like an address.".to_string())?;
    let fingerprint = Fingerprint::from_hex(&request.fingerprint).map_err(|e| e.sentence())?;

    let mut client = Client::connect(addr, fingerprint).map_err(|e| e.sentence())?;
    let session = client
        .sign_in(&request.device_token, &request.login, &request.password)
        .map_err(|e| e.sentence())?;

    *lock(&state.multi_user.client) = Some(client);
    *lock(&state.multi_user.connected_as) = Some(session.display_name);
    Ok(status_of(&state))
}

#[tauri::command]
pub fn disconnect_from_host(state: State<AppState>) -> HostingStatus {
    *lock(&state.multi_user.client) = None;
    *lock(&state.multi_user.connected_as) = None;
    status_of(&state)
}

// -------------------------------------------------------------- the door

/// Run a boundary command against whatever this machine is attached to.
///
/// Connected to someone else's budget, it goes over the wire. Otherwise it is
/// dispatched locally against the same registry, the same holds and the same
/// news — which is what makes a hold taken here block a laptop, and a change
/// made on a laptop show up here.
///
/// A command the registry does not know is reported as such rather than
/// guessed at; the frontend falls back to the host-only Tauri command of that
/// name.
#[tauri::command]
pub fn boundary_invoke(
    state: State<AppState>,
    command: String,
    args: serde_json::Value,
) -> Result<Response, String> {
    let request = Request::new(command, args);

    if let Some(client) = lock(&state.multi_user.client).as_mut() {
        return client.invoke(request).map_err(|e| e.sentence());
    }

    let db = db_of(&state)?;
    let actor = Actor::local_owner();
    let ctx = BoundaryCtx::new(&db, &actor, &state.shared);
    Ok(dispatch(&state.registry, &ctx, request))
}
