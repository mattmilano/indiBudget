//! The hosting side.
//!
//! One process owns the SQLite connection, full stop. Other machines reach it
//! over TLS on the LAN and every request they make goes through the same
//! boundary the local screens use, so no permission or rule can drift between
//! the two paths.
//!
//! Synchronous, one thread per connection. A household does not need an async
//! executor, and a dull transport is one that cannot surprise the data.

use chrono::Utc;
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use serde::Serialize;
use std::collections::HashMap;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::frame::{read_frame, write_frame};
use super::identity::HostIdentity;
use super::pairing::{
    attempt_pairing, device_for_token, Device, device_is_active, generate_code, register_device, touch_device, PairingOutcome,
    PairingWindow,
};
use super::protocol::{ClientMessage, ServerMessage};
use super::throttle::Throttle;
use crate::boundary::registry::{dispatch, BoundaryCtx, Registry};
use crate::boundary::users::{authenticate, standing};
use crate::boundary::news::Notice;
use crate::boundary::{Actor, BoundaryError, SharedState};
use crate::database::Database;

/// Everything a connection thread needs.
pub struct HostState {
    pub db: Arc<Database>,
    pub registry: Arc<Registry>,
    pub identity: HostIdentity,
    /// Edit holds and the news ring. Shared with the local session by holding
    /// the same Arc, so a hold taken at the hosting machine blocks a laptop and
    /// a change made on a laptop is heard here.
    pub shared: Arc<SharedState>,
    pairing: Mutex<Option<PairingWindow>>,
    throttle: Throttle,
    seats: Seats,
}

/// Someone signed in from another computer, as the Sharing screen lists them.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Seat {
    pub person: String,
    pub computer: String,
    pub connected_at: String,
    pub last_active_at: String,
}

/// Who is connected right now, one entry per signed-in connection.
///
/// The same person can be signed in from two computers at once. Their edit
/// holds are let go when the *last* of those goes, not the first: closing the
/// laptop lid must not free the budget they still have open on the desktop.
#[derive(Default)]
struct Seats {
    next: AtomicU64,
    held: Mutex<HashMap<u64, (String, Seat)>>,
}

impl Seats {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, (String, Seat)>> {
        self.held.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn take(&self, user_id: &str, person: &str, computer: &str) -> u64 {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let now = Utc::now().to_rfc3339();
        self.lock().insert(
            id,
            (
                user_id.to_string(),
                Seat {
                    person: person.to_string(),
                    computer: computer.to_string(),
                    connected_at: now.clone(),
                    last_active_at: now,
                },
            ),
        );
        id
    }

    fn touch(&self, id: u64, person: &str) {
        if let Some((_, seat)) = self.lock().get_mut(&id) {
            seat.person = person.to_string();
            seat.last_active_at = Utc::now().to_rfc3339();
        }
    }

    /// Give up a seat. Answers whether that person still has another.
    fn leave(&self, id: u64) -> bool {
        let mut held = self.lock();
        match held.remove(&id) {
            Some((user_id, _)) => held.values().any(|(other, _)| *other == user_id),
            None => false,
        }
    }

    fn list(&self) -> Vec<Seat> {
        let mut seats: Vec<Seat> = self.lock().values().map(|(_, s)| s.clone()).collect();
        seats.sort_by(|a, b| a.person.cmp(&b.person).then(a.computer.cmp(&b.computer)));
        seats
    }
}

impl HostState {
    pub fn new(
        db: Arc<Database>,
        registry: Arc<Registry>,
        identity: HostIdentity,
        shared: Arc<SharedState>,
    ) -> Self {
        HostState {
            db,
            registry,
            identity,
            shared,
            pairing: Mutex::new(None),
            throttle: Throttle::new(),
            seats: Seats::default(),
        }
    }

    /// Open a pairing window and return the code to show on screen.
    pub fn open_pairing(&self) -> String {
        let code = generate_code();
        let mut slot = self.lock_pairing();
        *slot = Some(PairingWindow::open(code.clone(), Instant::now()));
        code
    }

    /// Who is signed in from other computers right now.
    pub fn connected(&self) -> Vec<Seat> {
        self.seats.list()
    }

    pub fn close_pairing(&self) {
        *self.lock_pairing() = None;
    }

    pub fn is_pairing(&self) -> bool {
        self.lock_pairing()
            .as_ref()
            .map(|w| !w.is_expired(Instant::now()))
            .unwrap_or(false)
    }

    fn lock_pairing(&self) -> std::sync::MutexGuard<'_, Option<PairingWindow>> {
        self.pairing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Judge a pairing proof, updating the window.
    fn judge_pairing(&self, proof: &str, label: &str, replaces: Option<&str>) -> ServerMessage {
        let mut slot = self.lock_pairing();
        let Some(window) = slot.take() else {
            // Distinct from a wrong code on purpose: there is nothing to guess
            // at, and telling someone to check their code when no window is
            // open sends them round a loop that cannot end.
            return ServerMessage::refused(
                "This computer is not accepting new connections right now. \
                 Ask whoever hosts the budget to start pairing.",
            );
        };

        let (outcome, remaining) =
            attempt_pairing(window, proof, &self.identity.cert_der, Instant::now());
        *slot = remaining;
        drop(slot);

        match outcome {
            PairingOutcome::Accepted => {
                match self
                    .db
                    .with_connection(|conn| Ok(register_device(conn, label, replaces)))
                {
                    Ok(Ok((_, token))) => ServerMessage::Paired {
                        device_token: token,
                    },
                    Ok(Err(e)) => ServerMessage::refused(e.sentence()),
                    Err(e) => ServerMessage::refused(format!("Could not record this computer: {e}")),
                }
            }
            PairingOutcome::Rejected { attempts_remaining } => ServerMessage::refused(format!(
                "That code did not match. {attempts_remaining} more \
                 {} before pairing closes.",
                if attempts_remaining == 1 {
                    "try"
                } else {
                    "tries"
                }
            )),
            PairingOutcome::RejectedAndClosed => ServerMessage::refused(
                "That code did not match, and pairing has now closed. \
                 Ask whoever hosts the budget to start it again.",
            ),
        }
    }

    /// Check a device token and a person's credentials.
    fn judge_sign_in(
        &self,
        token: &str,
        login: &str,
        password: &str,
    ) -> Result<(Actor, Device), ServerMessage> {
        let now = Instant::now();

        if let Some(wait) = self.throttle.retry_after(login, now) {
            return Err(ServerMessage::throttled(
                "Too many sign-in attempts. Please wait a moment and try again.",
                wait.as_secs().max(1),
            ));
        }

        let device = self
            .db
            .with_connection(|conn| Ok(device_for_token(conn, token)))
            .map_err(|e| ServerMessage::refused(format!("Could not check this computer: {e}")))?;

        let device = match device {
            Ok(Some(device)) => device,
            Ok(None) => {
                // Revoked or never paired. A failed device check still counts
                // against the login's backoff, so this cannot be used as a
                // faster oracle than the password path.
                self.throttle.record_failure(login, now);
                return Err(ServerMessage::refused(
                    "This computer is not connected to that budget any more. \
                     It will need to be paired again.",
                ));
            }
            Err(e) => return Err(ServerMessage::refused(e.sentence())),
        };

        let outcome = self
            .db
            .with_connection(|conn| Ok(authenticate(conn, login, password)))
            .map_err(|e| ServerMessage::refused(format!("Could not check that sign-in: {e}")))?;

        match outcome {
            Ok(actor) => {
                self.throttle.record_success(login);
                let _ = self
                    .db
                    .with_connection(|conn| Ok(touch_device(conn, &device.id)));
                Ok((actor, device))
            }
            Err(e) => {
                self.throttle.record_failure(login, now);
                Err(ServerMessage::refused(e.sentence()))
            }
        }
    }
}

/// Who signed in on a connection, and from which computer.
///
/// The actor is a cache, refreshed from the file before every request — see
/// [`HostState::current_standing`].
struct Session {
    user_id: String,
    device_id: String,
    actor: Actor,
    seat: u64,
}

/// Why a session ended partway, in the person's words.
const ACCESS_REMOVED: &str = "Your access to this budget has been removed. \
    Ask whoever manages it if that is a mistake.";
const DEVICE_REVOKED: &str = "This computer is no longer connected to that budget. \
    It will need to be paired again.";

impl HostState {
    /// Re-check a session against the file. Deactivation, deletion, a change of
    /// access and a revoked computer all count from the very next request,
    /// rather than whenever that person happens to reconnect.
    fn current_standing(&self, session: &Session) -> Result<Actor, &'static str> {
        let device_ok = self
            .db
            .with_connection(|conn| Ok(device_is_active(conn, &session.device_id)))
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or(false);
        if !device_ok {
            return Err(DEVICE_REVOKED);
        }
        self.db
            .with_connection(|conn| Ok(standing(conn, &session.user_id)))
            .ok()
            .and_then(|r| r.ok())
            .flatten()
            .ok_or(ACCESS_REMOVED)
    }

    /// A connection is finished with: give up its seat, and let go of what the
    /// person held unless they are still here on another computer.
    fn leave(&self, session: &Session) {
        if !self.seats.leave(session.seat) {
            self.release_holds_of(&session.actor);
        }
    }

    fn release_holds_of(&self, actor: &Actor) {
        for key in self.shared.leases.release_everything_for(actor) {
            self.shared.news.publish(Notice::RecordFreed {
                area: key.kind.area(),
                record_kind: key.kind.label().to_string(),
                record_id: key.record_id,
            });
        }
    }
}

/// A host that is listening. Dropping it stops the listener.
pub struct RunningHost {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
    live: Arc<LiveConnections>,
}

/// Every connection currently being served, so stopping can end them.
///
/// Without this, "Stop hosting" only stopped *new* connections: computers that
/// had already joined went on reading and writing the budget through
/// connection threads that knew nothing about the stop.
#[derive(Default)]
struct LiveConnections {
    next: AtomicU64,
    streams: Mutex<HashMap<u64, TcpStream>>,
}

impl LiveConnections {
    fn add(&self, stream: &TcpStream) -> Option<u64> {
        let handle = stream.try_clone().ok()?;
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        self.streams
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id, handle);
        Some(id)
    }

    fn remove(&self, id: u64) {
        self.streams
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&id);
    }

    fn close_all(&self) {
        for (_, stream) in self.streams.lock().unwrap_or_else(|p| p.into_inner()).drain() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

impl RunningHost {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);

        // Unblock the accept() by connecting to ourselves once. The listener is
        // bound to 0.0.0.0, which Windows refuses as a destination — dialling
        // it there left accept() blocked and Stop hosting hung forever — so
        // dial loopback on the same port instead.
        let wake = if self.addr.ip().is_unspecified() {
            SocketAddr::from(([127, 0, 0, 1], self.addr.port()))
        } else {
            self.addr
        };
        let _ = TcpStream::connect_timeout(&wake, Duration::from_secs(2));
        if let Some(handle) = self.accept_thread.take() {
            let _ = handle.join();
        }

        self.live.close_all();
    }
}

impl Drop for RunningHost {
    fn drop(&mut self) {
        self.stop();
    }
}

fn tls_config(identity: &HostIdentity) -> Result<Arc<ServerConfig>, BoundaryError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| BoundaryError::internal(format!("Could not set up secure hosting: {e}")))?
        .with_no_client_auth()
        .with_single_cert(vec![identity.certificate()], identity.private_key()?)
        .map_err(|e| BoundaryError::internal(format!("Could not set up secure hosting: {e}")))?;
    Ok(Arc::new(config))
}

/// Start listening. `bind` may use port 0 to let the OS choose.
pub fn start(state: Arc<HostState>, bind: SocketAddr) -> Result<RunningHost, BoundaryError> {
    let config = tls_config(&state.identity)?;
    let listener = TcpListener::bind(bind).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            BoundaryError::invalid(format!(
                "Port {} is already in use on this computer — indiBudget may already be \
                 hosting in another window. Stop that, or choose a different port.",
                bind.port()
            ))
        } else {
            BoundaryError::internal(format!("Could not start hosting on port {}: {e}", bind.port()))
        }
    })?;
    let addr = listener
        .local_addr()
        .map_err(|e| BoundaryError::internal(format!("Could not read the hosting address: {e}")))?;

    let shutdown = Arc::new(AtomicBool::new(false));
    let accept_shutdown = Arc::clone(&shutdown);
    let live = Arc::new(LiveConnections::default());
    let accept_live = Arc::clone(&live);

    let accept_thread = std::thread::spawn(move || {
        for incoming in listener.incoming() {
            if accept_shutdown.load(Ordering::SeqCst) {
                break;
            }
            let Ok(stream) = incoming else { continue };
            let state = Arc::clone(&state);
            let config = Arc::clone(&config);
            let live = Arc::clone(&accept_live);
            std::thread::spawn(move || {
                let id = live.add(&stream);
                serve_connection(state, config, stream);
                if let Some(id) = id {
                    live.remove(id);
                }
            });
        }
    });

    Ok(RunningHost {
        addr,
        shutdown,
        accept_thread: Some(accept_thread),
        live,
    })
}

fn serve_connection(state: Arc<HostState>, config: Arc<ServerConfig>, stream: TcpStream) {
    let Ok(conn) = ServerConnection::new(config) else {
        return;
    };
    let mut tls = StreamOwned::new(conn, stream);

    // Nobody is signed in until they say who they are.
    let mut session: Option<Session> = None;

    loop {
        let Ok(raw) = read_frame(&mut tls) else {
            break; // peer went away, or a bad frame — either way, done
        };

        let reply = match serde_json::from_str::<ClientMessage>(&raw) {
            Ok(message) => handle_message(&state, &mut session, message),
            Err(e) => ServerMessage::refused(format!("That message could not be read: {e}")),
        };

        let Ok(encoded) = serde_json::to_string(&reply) else {
            break;
        };
        if write_frame(&mut tls, &encoded).is_err() {
            break;
        }
    }

    // A machine that closed its lid should not leave the grocery budget held
    // for the rest of the lease. Passive expiry would clear it eventually;
    // this clears it now.
    if let Some(session) = session.as_ref() {
        state.leave(session);
    }
}

fn handle_message(
    state: &Arc<HostState>,
    session: &mut Option<Session>,
    message: ClientMessage,
) -> ServerMessage {
    match message {
        ClientMessage::Pair { proof, label, replaces } => {
            if session.is_some() {
                return ServerMessage::refused("This computer is already connected.");
            }
            state.judge_pairing(&proof, &label, replaces.as_deref())
        }

        ClientMessage::Authenticate {
            device_token,
            login,
            password,
        } => match state.judge_sign_in(&device_token, &login, &password) {
            Ok((signed_in, device)) => {
                let reply = ServerMessage::Authenticated {
                    display_name: signed_in.display_name.clone(),
                    is_owner: signed_in.is_owner,
                };
                // Signing in again on the same connection gives up the old seat.
                if let Some(previous) = session.take() {
                    state.leave(&previous);
                }
                let seat = state
                    .seats
                    .take(&signed_in.user_id, &signed_in.display_name, &device.label);
                *session = Some(Session {
                    user_id: signed_in.user_id.clone(),
                    device_id: device.id,
                    actor: signed_in,
                    seat,
                });
                reply
            }
            Err(refusal) => refusal,
        },

        ClientMessage::Invoke { request } => {
            let Some(current) = session.as_mut() else {
                return ServerMessage::refused("Please sign in first.");
            };

            // The actor built at sign-in is a claim to re-check, not a fact.
            match state.current_standing(current) {
                Ok(fresh) => {
                    state.seats.touch(current.seat, &fresh.display_name);
                    current.actor = fresh;
                }
                Err(why) => {
                    // Off the list at once, and everything they held let go:
                    // their other computers are refused on their next click too.
                    let ended = session.take().expect("checked above");
                    state.seats.leave(ended.seat);
                    state.release_holds_of(&ended.actor);
                    return ServerMessage::refused(why);
                }
            }

            // The identity used here comes from the connection, never from the
            // request body, so no client can name itself administrator.
            let ctx = BoundaryCtx::new(&state.db, &current.actor, &state.shared);
            ServerMessage::Reply {
                response: dispatch(&state.registry, &ctx, request),
            }
        }
    }
}
