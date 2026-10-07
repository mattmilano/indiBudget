//! The connecting side.

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, Error as TlsError, SignatureScheme, StreamOwned};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::frame::{read_frame, write_frame, FrameError};
use super::identity::{Fingerprint, PinnedServerCertVerifier};
use super::pairing::pairing_proof;
use super::protocol::{ClientMessage, ServerMessage};
use crate::boundary::news::Mark;
use crate::boundary::{BoundaryError, Request, Response};

/// How long to wait for the host to accept a connection.
///
/// Firewalls often drop unwanted traffic rather than refusing it, and then an
/// operating system will keep trying to connect for a minute or more before
/// giving up. Nobody should wait that long to learn the address is wrong.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long to wait for any one reply once connected.
///
/// Generous next to normal replies, which arrive in milliseconds, but bounded:
/// a host that went to sleep mid-conversation must produce an error, not an
/// endless wait.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(20);

fn waited_too_long(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    )
}

fn no_answer(addr: SocketAddr) -> BoundaryError {
    BoundaryError::invalid(format!(
        "The computer at {addr} did not answer. Check that it is hosting, that it \
         is awake, and that its firewall allows incoming TCP connections on port {}.",
        addr.port()
    ))
}

/// Turn a failure partway through a conversation into a sentence.
fn lost(addr: SocketAddr, e: &std::io::Error) -> BoundaryError {
    if waited_too_long(e) {
        no_answer(addr)
    } else {
        BoundaryError::invalid(format!(
            "The connection to the computer hosting the budget was lost: {e}"
        ))
    }
}

/// The name presented in the handshake. It is never checked — pinning replaces
/// hostname verification — but the API requires one.
const HOST_NAME: &str = "indibudget-host";

/// Records the certificate a host presented without judging it.
///
/// Used **only** for the very first pairing connection, where there is by
/// definition no pin yet. This is not trust-on-first-use in the usual sense:
/// nothing is trusted as a result of connecting. The pairing proof the client
/// then sends is computed over the certificate captured here, so a
/// machine-in-the-middle that terminated this connection with its own
/// certificate produces a proof bound to the wrong DER and is refused by the
/// real host. The short code the person typed is what makes that check
/// meaningful, and it never travels.
#[derive(Debug)]
struct CaptureCertVerifier {
    captured: Mutex<Option<Vec<u8>>>,
    provider: Arc<CryptoProvider>,
}

impl CaptureCertVerifier {
    fn new(provider: Arc<CryptoProvider>) -> Self {
        CaptureCertVerifier {
            captured: Mutex::new(None),
            provider,
        }
    }

    fn captured(&self) -> Option<Vec<u8>> {
        self.captured
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

impl ServerCertVerifier for CaptureCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        *self.captured.lock().unwrap_or_else(|p| p.into_inner()) =
            Some(end_entity.as_ref().to_vec());
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// A connection to a host.
pub struct Client {
    tls: StreamOwned<ClientConnection, TcpStream>,
    addr: SocketAddr,
    /// Set once a request failed partway. The protocol is strict
    /// request/reply, so after a timeout a late reply could arrive as the
    /// answer to the *next* request; the only safe thing is to stop using this
    /// connection.
    broken: bool,
    /// The fingerprint actually presented, for a caller that is pairing.
    observed_fingerprint: Option<Fingerprint>,
}

fn connect_with_verifier(
    addr: SocketAddr,
    verifier: Arc<dyn ServerCertVerifier>,
    provider: Arc<CryptoProvider>,
) -> Result<StreamOwned<ClientConnection, TcpStream>, BoundaryError> {
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| BoundaryError::internal(format!("Could not set up a secure connection: {e}")))?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();

    let name = ServerName::try_from(HOST_NAME)
        .map_err(|e| BoundaryError::internal(format!("Bad host name: {e}")))?;

    let conn = ClientConnection::new(Arc::new(config), name).map_err(|e| {
        BoundaryError::internal(format!("Could not start a secure connection: {e}"))
    })?;

    let socket = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(|e| {
        if waited_too_long(&e) {
            no_answer(addr)
        } else if e.kind() == std::io::ErrorKind::ConnectionRefused {
            BoundaryError::invalid(format!(
                "Nothing at {addr} is accepting connections. Check that the other \
                 computer is hosting, and that the address matches the one on its \
                 Sharing screen."
            ))
        } else {
            BoundaryError::invalid(format!(
                "Could not reach the computer hosting the budget at {addr}: {e}"
            ))
        }
    })?;

    // Without these, a host that accepts and then goes quiet leaves the reader
    // waiting forever — which, before commands moved off the main thread,
    // froze the whole app.
    for result in [
        socket.set_read_timeout(Some(REPLY_TIMEOUT)),
        socket.set_write_timeout(Some(REPLY_TIMEOUT)),
    ] {
        result.map_err(|e| BoundaryError::internal(format!("Could not configure the connection: {e}")))?;
    }
    let _ = socket.set_nodelay(true);

    Ok(StreamOwned::new(conn, socket))
}

impl Client {
    /// Connect to a host whose fingerprint is already known.
    ///
    /// The pin is enforced during the handshake, before a single byte of
    /// application data is sent.
    pub fn connect(addr: SocketAddr, expected: Fingerprint) -> Result<Self, BoundaryError> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = Arc::new(PinnedServerCertVerifier::new(
            expected,
            Arc::clone(&provider),
        ));
        let tls = connect_with_verifier(addr, verifier, provider)?;
        Ok(Client {
            tls,
            addr,
            broken: false,
            observed_fingerprint: Some(expected),
        })
    }

    /// Connect for the sole purpose of pairing, capturing the certificate so
    /// the proof can be bound to it.
    pub fn connect_for_pairing(addr: SocketAddr) -> Result<Self, BoundaryError> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = Arc::new(CaptureCertVerifier::new(Arc::clone(&provider)));
        let mut tls = connect_with_verifier(addr, Arc::clone(&verifier) as Arc<dyn ServerCertVerifier>, provider)?;

        // Force the handshake so the certificate is available before anything
        // is asked of the connection.
        tls.conn
            .complete_io(&mut tls.sock)
            .map_err(|e| lost(addr, &e))?;

        let captured = verifier.captured().ok_or_else(|| {
            BoundaryError::internal("That computer did not present an identity.")
        })?;

        Ok(Client {
            tls,
            addr,
            broken: false,
            observed_fingerprint: Some(Fingerprint::of_certificate(&captured)),
        })
    }

    /// Whether a request failed partway, making this connection unusable.
    pub fn is_broken(&self) -> bool {
        self.broken
    }

    /// The fingerprint of the host on the other end. Store this after a
    /// successful pairing; it is what every later connection is checked against.
    pub fn host_fingerprint(&self) -> Option<Fingerprint> {
        self.observed_fingerprint
    }

    /// The certificate the host actually presented, for binding a proof.
    fn peer_certificate(&self) -> Result<Vec<u8>, BoundaryError> {
        self.tls
            .conn
            .peer_certificates()
            .and_then(|certs| certs.first())
            .map(|c| c.as_ref().to_vec())
            .ok_or_else(|| BoundaryError::internal("That computer did not present an identity."))
    }

    fn exchange(&mut self, message: ClientMessage) -> Result<ServerMessage, BoundaryError> {
        let encoded = serde_json::to_string(&message)
            .map_err(|e| BoundaryError::internal(format!("Could not prepare that request: {e}")))?;
        if self.broken {
            return Err(BoundaryError::invalid(
                "The connection to the computer hosting the budget was lost. Sign in again.",
            ));
        }
        let addr = self.addr;
        let fail = |client: &mut Client, e: FrameError| {
            client.broken = true;
            match e {
                FrameError::Io(io) => lost(addr, &io),
                FrameError::Truncated => BoundaryError::invalid(
                    "The computer hosting the budget closed the connection.",
                ),
                other => BoundaryError::invalid(format!("The connection failed: {other}")),
            }
        };
        if let Err(e) = write_frame(&mut self.tls, &encoded) {
            return Err(fail(self, e));
        }
        let raw = match read_frame(&mut self.tls) {
            Ok(raw) => raw,
            Err(e) => return Err(fail(self, e)),
        };
        serde_json::from_str(&raw)
            .map_err(|e| BoundaryError::internal(format!("Could not read the reply: {e}")))
    }

    /// Offer the code a person read off the host's screen.
    ///
    /// The proof is computed over the certificate this connection actually
    /// received, which is what defeats a machine-in-the-middle.
    pub fn pair(&mut self, code: &str, label: &str) -> Result<String, BoundaryError> {
        self.pair_again(code, label, None)
    }

    /// Pair, handing back the token from an earlier pairing with this same
    /// host so the host replaces that entry instead of adding a second one.
    ///
    /// Only pass `replaces` once [`Client::host_fingerprint`] has been checked
    /// against the identity remembered from that earlier pairing. This
    /// connection's certificate is not pinned, and the old token must never go
    /// to a computer that has not proved it is the same host.
    pub fn pair_again(
        &mut self,
        code: &str,
        label: &str,
        replaces: Option<&str>,
    ) -> Result<String, BoundaryError> {
        let cert = self.peer_certificate()?;
        let proof = pairing_proof(code, &cert);

        match self.exchange(ClientMessage::Pair {
            proof,
            label: label.to_string(),
            replaces: replaces.map(str::to_string),
        })? {
            ServerMessage::Paired { device_token } => Ok(device_token),
            ServerMessage::Refused { sentence, .. } => Err(BoundaryError::invalid(sentence)),
            other => Err(BoundaryError::internal(format!(
                "Unexpected reply while pairing: {other:?}"
            ))),
        }
    }

    /// Present the machine's token and a person's credentials.
    pub fn sign_in(
        &mut self,
        device_token: &str,
        login: &str,
        password: &str,
    ) -> Result<SignedIn, BoundaryError> {
        match self.exchange(ClientMessage::Authenticate {
            device_token: device_token.to_string(),
            login: login.to_string(),
            password: password.to_string(),
        })? {
            ServerMessage::Authenticated {
                display_name,
                is_owner,
                watch_ticket,
            } => Ok(SignedIn {
                display_name,
                is_owner,
                watch_ticket,
            }),
            ServerMessage::Refused {
                sentence,
                retry_after_secs,
            } => Err(match retry_after_secs {
                Some(seconds) => BoundaryError::invalid(format!("{sentence} ({seconds}s)")),
                None => BoundaryError::invalid(sentence),
            }),
            other => Err(BoundaryError::internal(format!(
                "Unexpected reply while signing in: {other:?}"
            ))),
        }
    }

    /// Run a boundary command on the host.
    pub fn invoke(&mut self, request: Request) -> Result<Response, BoundaryError> {
        match self.exchange(ClientMessage::Invoke { request })? {
            ServerMessage::Reply { response } => Ok(response),
            // A refusal of the request as a whole, rather than an answer to it,
            // means the host ended this session — access removed, computer
            // revoked. Carrying on would only collect "please sign in first".
            ServerMessage::Refused { sentence, .. } => {
                self.broken = true;
                Err(BoundaryError::invalid(sentence))
            }
            other => Err(BoundaryError::internal(format!(
                "Unexpected reply to a request: {other:?}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedIn {
    pub display_name: String,
    pub is_owner: bool,
    /// Opens one nudge connection; see [`Watcher`]. Absent from a host too old
    /// to push, which leaves the five-second beat to do the work alone.
    pub watch_ticket: Option<String>,
}

/// How long the nudge connection waits to hear anything before deciding the
/// host has gone. Well past the host's own [`super::host::NUDGE_EVERY`].
pub const WATCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Something to call when the host says the budget changed.
pub type OnNudge = Arc<dyn Fn(&Mark) + Send + Sync>;

/// A second connection to the host on which it says, unasked, that something
/// changed — so a transaction added on the laptop shows on the desktop at once
/// rather than at the next five-second beat.
///
/// Kept apart from [`Client`] because that connection is strict request/reply:
/// a nudge arriving on it could be read as the answer to a request. The nudge
/// carries only a mark; what changed is asked for through the ordinary
/// catch-up, which filters it by what this person may see.
///
/// Nothing depends on this working. If it fails or drops, the beat still
/// catches everything up, a few seconds later.
pub struct Watcher {
    socket: TcpStream,
    thread: Option<std::thread::JoinHandle<()>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl Watcher {
    /// Open the nudge connection with a ticket from sign-in, and start
    /// listening on a thread of its own.
    pub fn start(
        addr: SocketAddr,
        expected: Fingerprint,
        ticket: &str,
        on_nudge: OnNudge,
    ) -> Result<Self, BoundaryError> {
        let mut client = Client::connect(addr, expected)?;
        match client.exchange(ClientMessage::Watch {
            ticket: ticket.to_string(),
        })? {
            ServerMessage::Watching => {}
            ServerMessage::Refused { sentence, .. } => return Err(BoundaryError::invalid(sentence)),
            other => {
                return Err(BoundaryError::internal(format!(
                    "Unexpected reply while asking to hear about changes: {other:?}"
                )))
            }
        }

        let mut tls = client.tls;
        let socket = tls
            .sock
            .try_clone()
            .map_err(|e| BoundaryError::internal(format!("Could not keep the connection: {e}")))?;
        let _ = tls.sock.set_read_timeout(Some(WATCH_TIMEOUT));

        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let still = Arc::clone(&running);
        let thread = std::thread::spawn(move || {
            let mut last: Option<Mark> = None;
            // Ends when the host goes quiet for too long, closes the
            // connection, or this side shuts the socket.
            while let Ok(raw) = read_frame(&mut tls) {
                let Ok(ServerMessage::Nudge { mark }) = serde_json::from_str(&raw) else {
                    break;
                };
                if last.as_ref() != Some(&mark) {
                    on_nudge(&mark);
                    last = Some(mark);
                }
            }
            still.store(false, std::sync::atomic::Ordering::SeqCst);
        });

        Ok(Watcher {
            socket,
            thread: Some(thread),
            running,
        })
    }

    /// Whether the host is still sending nudges.
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
