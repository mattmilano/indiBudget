//! The connecting side.

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, Error as TlsError, SignatureScheme, StreamOwned};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};

use super::frame::{read_frame, write_frame};
use super::identity::{Fingerprint, PinnedServerCertVerifier};
use super::pairing::pairing_proof;
use super::protocol::{ClientMessage, ServerMessage};
use crate::boundary::{BoundaryError, Request, Response};

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

    let socket = TcpStream::connect(addr).map_err(|e| {
        BoundaryError::invalid(format!(
            "Could not reach the computer hosting the budget at {addr}: {e}"
        ))
    })?;

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
            .map_err(|e| BoundaryError::invalid(format!("Could not reach that computer: {e}")))?;

        let captured = verifier.captured().ok_or_else(|| {
            BoundaryError::internal("That computer did not present an identity.")
        })?;

        Ok(Client {
            tls,
            observed_fingerprint: Some(Fingerprint::of_certificate(&captured)),
        })
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
        write_frame(&mut self.tls, &encoded)
            .map_err(|e| BoundaryError::invalid(format!("The connection failed: {e}")))?;
        let raw = read_frame(&mut self.tls)
            .map_err(|e| BoundaryError::invalid(format!("The connection failed: {e}")))?;
        serde_json::from_str(&raw)
            .map_err(|e| BoundaryError::internal(format!("Could not read the reply: {e}")))
    }

    /// Offer the code a person read off the host's screen.
    ///
    /// The proof is computed over the certificate this connection actually
    /// received, which is what defeats a machine-in-the-middle.
    pub fn pair(&mut self, code: &str, label: &str) -> Result<String, BoundaryError> {
        let cert = self.peer_certificate()?;
        let proof = pairing_proof(code, &cert);

        match self.exchange(ClientMessage::Pair {
            proof,
            label: label.to_string(),
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
            } => Ok(SignedIn {
                display_name,
                is_owner,
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
            ServerMessage::Refused { sentence, .. } => Err(BoundaryError::invalid(sentence)),
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
}
