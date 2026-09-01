//! Host identity and certificate pinning.
//!
//! A machine on a home network has no DNS name a certificate authority would
//! vouch for, so the usual chain-and-name check has nothing to check against.
//! Instead the host presents a self-signed certificate, the client learns its
//! SHA-256 fingerprint once during pairing, and every later connection is
//! refused unless the certificate presented hashes to that same value.
//!
//! What is given up: chain validation and hostname matching. What is kept:
//! the TLS signature check — the peer must still hold the private key for the
//! certificate it presents — plus an exact-identity check that a CA could not
//! have given us anyway on an unnamed LAN machine.

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error as TlsError, SignatureScheme};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::boundary::BoundaryError;

/// The name on the self-signed certificate. It is never checked — pinning
/// replaces name verification — but a certificate needs some subject.
const HOST_CERT_NAME: &str = "indibudget-host";

const SETTING_CERT: &str = "host_certificate_der";
const SETTING_KEY: &str = "host_private_key_der";

/// A SHA-256 certificate fingerprint: the thing a client pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    pub fn of_certificate(cert_der: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(cert_der);
        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        Fingerprint(out)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> Result<Self, BoundaryError> {
        let bytes = hex::decode(s.trim())
            .map_err(|_| BoundaryError::invalid("That connection fingerprint is not readable."))?;
        if bytes.len() != 32 {
            return Err(BoundaryError::invalid(
                "That connection fingerprint is the wrong length.",
            ));
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(&bytes);
        Ok(Fingerprint(out))
    }

    /// Grouped hex, for showing a person the code to compare by eye.
    pub fn display_groups(self) -> String {
        self.to_hex()
            .as_bytes()
            .chunks(4)
            .map(|c| String::from_utf8_lossy(c).to_uppercase())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// The host's certificate and private key.
#[derive(Debug, Clone)]
pub struct HostIdentity {
    pub cert_der: Vec<u8>,
    pub key_der: Vec<u8>,
}

impl HostIdentity {
    pub fn generate() -> Result<Self, BoundaryError> {
        let generated = rcgen::generate_simple_self_signed(vec![HOST_CERT_NAME.to_string()])
            .map_err(|e| {
                BoundaryError::internal(format!("Could not create this computer's identity: {e}"))
            })?;
        Ok(HostIdentity {
            cert_der: generated.cert.der().to_vec(),
            key_der: generated.key_pair.serialize_der(),
        })
    }

    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of_certificate(&self.cert_der)
    }

    pub fn certificate(&self) -> CertificateDer<'static> {
        CertificateDer::from(self.cert_der.clone())
    }

    pub fn private_key(&self) -> Result<PrivateKeyDer<'static>, BoundaryError> {
        PrivateKeyDer::try_from(self.key_der.clone()).map_err(|e| {
            BoundaryError::internal(format!("This computer's private key is unreadable: {e}"))
        })
    }

    /// Load the stored identity, creating one on first use.
    ///
    /// The identity must survive a restart. If the host generated a fresh
    /// certificate each time it started, every paired machine's pin would stop
    /// matching and everyone would have to pair again after a reboot.
    pub fn load_or_create(conn: &Connection) -> Result<Self, BoundaryError> {
        let stored_cert = read_setting(conn, SETTING_CERT)?;
        let stored_key = read_setting(conn, SETTING_KEY)?;

        if let (Some(cert), Some(key)) = (stored_cert, stored_key) {
            let cert_der = base64_decode(&cert)?;
            let key_der = base64_decode(&key)?;
            return Ok(HostIdentity { cert_der, key_der });
        }

        let identity = HostIdentity::generate()?;
        write_setting(conn, SETTING_CERT, &base64_encode(&identity.cert_der))?;
        write_setting(conn, SETTING_KEY, &base64_encode(&identity.key_der))?;
        Ok(identity)
    }
}

fn read_setting(conn: &Connection, key: &str) -> Result<Option<String>, BoundaryError> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| BoundaryError::internal(format!("Could not read this computer's identity: {e}")))
}

fn write_setting(conn: &Connection, key: &str, value: &str) -> Result<(), BoundaryError> {
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
        rusqlite::params![key, value, chrono::Utc::now().to_rfc3339()],
    )
    .map_err(|e| BoundaryError::internal(format!("Could not store this computer's identity: {e}")))?;
    Ok(())
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, BoundaryError> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD
        .decode(s)
        .map_err(|e| BoundaryError::internal(format!("This computer's identity is corrupt: {e}")))
}

/// A server-certificate verifier that trusts exactly one fingerprint.
///
/// Chain building and hostname matching are skipped on purpose — see the module
/// comment. The signature checks below are *not* skipped: they are delegated to
/// the crypto provider unchanged, so a peer that does not hold the private key
/// for the certificate it presented still fails the handshake.
#[derive(Debug)]
pub struct PinnedServerCertVerifier {
    expected: Fingerprint,
    provider: Arc<CryptoProvider>,
}

impl PinnedServerCertVerifier {
    pub fn new(expected: Fingerprint, provider: Arc<CryptoProvider>) -> Self {
        PinnedServerCertVerifier { expected, provider }
    }
}

impl ServerCertVerifier for PinnedServerCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let presented = Fingerprint::of_certificate(end_entity.as_ref());
        if presented == self.expected {
            Ok(ServerCertVerified::assertion())
        } else {
            // Deliberately generic to the TLS layer; the client turns this into
            // a sentence that tells the person what it means.
            Err(TlsError::General(
                "the host's identity does not match the one this computer paired with".into(),
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    #[test]
    fn a_generated_identity_has_a_usable_certificate_and_key() {
        let identity = HostIdentity::generate().unwrap();
        assert!(!identity.cert_der.is_empty());
        assert!(!identity.key_der.is_empty());
        assert!(identity.private_key().is_ok());
    }

    #[test]
    fn two_identities_differ() {
        let first = HostIdentity::generate().unwrap();
        let second = HostIdentity::generate().unwrap();
        assert_ne!(first.fingerprint(), second.fingerprint());
    }

    #[test]
    fn a_fingerprint_is_stable_for_the_same_certificate() {
        let identity = HostIdentity::generate().unwrap();
        assert_eq!(identity.fingerprint(), identity.fingerprint());
        assert_eq!(
            identity.fingerprint(),
            Fingerprint::of_certificate(&identity.cert_der)
        );
    }

    #[test]
    fn fingerprints_round_trip_through_hex() {
        let identity = HostIdentity::generate().unwrap();
        let fingerprint = identity.fingerprint();
        let hex = fingerprint.to_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(Fingerprint::from_hex(&hex).unwrap(), fingerprint);
    }

    #[test]
    fn malformed_fingerprints_are_refused_with_a_sentence() {
        assert!(Fingerprint::from_hex("not hex at all").is_err());
        assert!(Fingerprint::from_hex("abcd").is_err(), "too short");
        assert!(Fingerprint::from_hex(&"ab".repeat(33)).is_err(), "too long");
    }

    #[test]
    fn display_groups_are_readable_aloud() {
        let identity = HostIdentity::generate().unwrap();
        let shown = identity.fingerprint().display_groups();
        assert!(shown.contains(' '));
        assert_eq!(shown.replace(' ', "").len(), 64);
        assert_eq!(shown, shown.to_uppercase());
    }

    /// If the host minted a new certificate on every start, every paired
    /// machine would have to pair again after a reboot.
    #[test]
    fn the_host_identity_survives_a_restart() {
        let db = Database::in_memory().unwrap();
        let first = db
            .with_connection(|conn| Ok(HostIdentity::load_or_create(conn).unwrap()))
            .unwrap();
        let second = db
            .with_connection(|conn| Ok(HostIdentity::load_or_create(conn).unwrap()))
            .unwrap();

        assert_eq!(
            first.fingerprint(),
            second.fingerprint(),
            "the host minted a new identity instead of loading the stored one"
        );
        assert_eq!(first.cert_der, second.cert_der);
        assert_eq!(first.key_der, second.key_der);
    }

    #[test]
    fn a_stored_identity_is_not_kept_in_plain_der_text() {
        // Stored base64-encoded, so a casual read of app_settings does not
        // hand someone a directly usable key file.
        let db = Database::in_memory().unwrap();
        db.with_connection(|conn| {
            let identity = HostIdentity::load_or_create(conn).unwrap();
            let stored: String = conn
                .query_row(
                    "SELECT value FROM app_settings WHERE key = ?1",
                    [SETTING_KEY],
                    |row| row.get(0),
                )
                .unwrap();
            assert_ne!(stored.as_bytes(), identity.key_der.as_slice());
            assert_eq!(base64_decode(&stored).unwrap(), identity.key_der);
            Ok(())
        })
        .unwrap();
    }
}
