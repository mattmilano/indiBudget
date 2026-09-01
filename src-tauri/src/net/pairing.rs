//! Pairing a machine, and the device tokens that result.
//!
//! Pairing answers one question — "was this machine deliberately added?" — and
//! nothing else. Who is sitting at it is a separate question with a separate
//! credential, asked at sign-in.
//!
//! The pairing proof is bound to the host's certificate:
//!
//! ```text
//! proof = SHA256(normalized_code || host_certificate_der)
//! ```
//!
//! The client computes it over the certificate it actually received during the
//! handshake. A machine-in-the-middle that terminates TLS with its own
//! certificate therefore earns a proof bound to the wrong DER, and when it
//! forwards that proof to the real host it does not match. Binding the proof to
//! the certificate is what makes a short, human-typed code safe to use on a
//! network nobody has verified yet.

use chrono::Utc;
use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::boundary::BoundaryError;

/// How long a pairing window stays open.
pub const PAIRING_WINDOW: Duration = Duration::from_secs(5 * 60);

/// Wrong guesses allowed before the window closes.
pub const MAX_PAIRING_ATTEMPTS: u32 = 5;

const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const CODE_LENGTH: usize = 8;

/// Strip the punctuation people add when reading a code aloud, and fold case.
///
/// Someone reads "K7P2-9WMX" off a screen and types "k7p2 9wmx"; both must
/// reach the same proof or pairing fails for no reason a person can see.
pub fn normalize_code(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// A code that is unambiguous when read aloud: no O/0, no I/1.
pub fn generate_code() -> String {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; CODE_LENGTH];
    rng.fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|b| CODE_ALPHABET[*b as usize % CODE_ALPHABET.len()] as char)
        .collect()
}

/// `SHA256(normalized_code || cert_der)`, hex encoded.
pub fn pairing_proof(code: &str, cert_der: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(normalize_code(code).as_bytes());
    hasher.update(cert_der);
    hex::encode(hasher.finalize())
}

fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn generate_device_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// The host's open pairing window. Lives in memory only: a crash must not
/// leave a machine able to pair itself.
#[derive(Debug)]
pub struct PairingWindow {
    code: String,
    opened_at: Instant,
    attempts_remaining: u32,
}

impl PairingWindow {
    pub fn open(code: String, now: Instant) -> Self {
        PairingWindow {
            code,
            opened_at: now,
            attempts_remaining: MAX_PAIRING_ATTEMPTS,
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn is_expired(&self, now: Instant) -> bool {
        now.duration_since(self.opened_at) >= PAIRING_WINDOW
    }

    pub fn attempts_remaining(&self) -> u32 {
        self.attempts_remaining
    }
}

/// What a pairing attempt did to the window.
#[derive(Debug, PartialEq, Eq)]
pub enum PairingOutcome {
    /// The proof matched. The window is consumed.
    Accepted,
    /// Wrong proof, and the window is still open for another try.
    Rejected { attempts_remaining: u32 },
    /// Wrong proof, and that was the last try — the window is now closed.
    RejectedAndClosed,
}

/// Check a proof against an open window.
///
/// Returns the outcome and the window's new state; `None` means the window is
/// gone and the host must stop advertising itself as pairing.
///
/// The last wrong guess **closes the window**. If it stayed open-but-exhausted,
/// the next caller — even one holding the right code — would be told to check
/// their code rather than that nothing is pairing, and would keep retrying a
/// window that can never accept them.
pub fn attempt_pairing(
    window: PairingWindow,
    offered_proof: &str,
    host_cert_der: &[u8],
    now: Instant,
) -> (PairingOutcome, Option<PairingWindow>) {
    if window.is_expired(now) {
        return (PairingOutcome::RejectedAndClosed, None);
    }

    let expected = pairing_proof(&window.code, host_cert_der);
    if constant_time_eq(offered_proof.as_bytes(), expected.as_bytes()) {
        return (PairingOutcome::Accepted, None);
    }

    let remaining = window.attempts_remaining.saturating_sub(1);
    if remaining == 0 {
        (PairingOutcome::RejectedAndClosed, None)
    } else {
        (
            PairingOutcome::Rejected {
                attempts_remaining: remaining,
            },
            Some(PairingWindow {
                code: window.code,
                opened_at: window.opened_at,
                attempts_remaining: remaining,
            }),
        )
    }
}

/// Compare without an early return, so timing does not leak how much of a
/// proof was correct.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// A paired machine, as the hosting screen lists them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub label: String,
    pub paired_at: String,
    pub last_seen_at: Option<String>,
    pub is_revoked: bool,
}

fn map_db(e: rusqlite::Error) -> BoundaryError {
    BoundaryError::internal(format!("Could not reach the list of paired computers: {e}"))
}

/// Record a newly paired machine and hand back its token.
///
/// The token is returned to the caller exactly once and only its hash is
/// stored, so a copy of the host database yields no usable credential.
pub fn register_device(conn: &Connection, label: &str) -> Result<(Device, String), BoundaryError> {
    let label = label.trim();
    let label = if label.is_empty() {
        "A computer"
    } else {
        label
    };

    let token = generate_device_token();
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO devices (id, label, token_hash, paired_at, last_seen_at, is_revoked)
         VALUES (?1, ?2, ?3, ?4, NULL, 0)",
        params![&id, label, token_hash(&token), &now],
    )
    .map_err(map_db)?;

    Ok((
        Device {
            id,
            label: label.to_string(),
            paired_at: now,
            last_seen_at: None,
            is_revoked: false,
        },
        token,
    ))
}

/// Look up a device by its token, refusing revoked ones.
pub fn device_for_token(conn: &Connection, token: &str) -> Result<Option<Device>, BoundaryError> {
    conn.query_row(
        "SELECT id, label, paired_at, last_seen_at, is_revoked
         FROM devices WHERE token_hash = ?1 AND is_revoked = 0",
        [token_hash(token)],
        |row| {
            Ok(Device {
                id: row.get(0)?,
                label: row.get(1)?,
                paired_at: row.get(2)?,
                last_seen_at: row.get(3)?,
                is_revoked: row.get::<_, i64>(4)? != 0,
            })
        },
    )
    .optional()
    .map_err(map_db)
}

pub fn touch_device(conn: &Connection, device_id: &str) -> Result<(), BoundaryError> {
    conn.execute(
        "UPDATE devices SET last_seen_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), device_id],
    )
    .map_err(map_db)?;
    Ok(())
}

pub fn list_devices(conn: &Connection) -> Result<Vec<Device>, BoundaryError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, label, paired_at, last_seen_at, is_revoked
             FROM devices ORDER BY paired_at DESC",
        )
        .map_err(map_db)?;
    let devices = stmt
        .query_map([], |row| {
            Ok(Device {
                id: row.get(0)?,
                label: row.get(1)?,
                paired_at: row.get(2)?,
                last_seen_at: row.get(3)?,
                is_revoked: row.get::<_, i64>(4)? != 0,
            })
        })
        .map_err(map_db)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_db)?;
    Ok(devices)
}

/// Revoke a machine. Takes effect at its next connection — a session already
/// running finishes what it is doing. The lever is for the machine, not the
/// moment.
pub fn revoke_device(conn: &Connection, device_id: &str) -> Result<(), BoundaryError> {
    let changed = conn
        .execute(
            "UPDATE devices SET is_revoked = 1 WHERE id = ?1",
            [device_id],
        )
        .map_err(map_db)?;
    if changed == 0 {
        return Err(BoundaryError::invalid(
            "That computer is no longer in the list.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::net::identity::HostIdentity;

    fn now() -> Instant {
        Instant::now()
    }

    #[test]
    fn codes_avoid_characters_that_are_ambiguous_aloud() {
        for _ in 0..200 {
            let code = generate_code();
            assert_eq!(code.len(), CODE_LENGTH);
            for c in code.chars() {
                assert!(
                    !"OI01".contains(c),
                    "code {code} contains an ambiguous character"
                );
            }
        }
    }

    #[test]
    fn codes_are_not_predictable() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            seen.insert(generate_code());
        }
        assert!(seen.len() > 190, "codes repeated far too often");
    }

    #[test]
    fn a_code_typed_with_spaces_or_lowercase_still_matches() {
        let identity = HostIdentity::generate().unwrap();
        let expected = pairing_proof("K7P2WMX9", &identity.cert_der);

        for typed in ["k7p2wmx9", "K7P2-WMX9", " k7p2 wmx9 ", "K7P2 WMX9"] {
            assert_eq!(
                pairing_proof(typed, &identity.cert_der),
                expected,
                "typing it as {typed:?} should have worked"
            );
        }
    }

    /// The property the whole pairing design rests on.
    #[test]
    fn a_proof_bound_to_another_certificate_does_not_match() {
        let real_host = HostIdentity::generate().unwrap();
        let impostor = HostIdentity::generate().unwrap();
        let code = "K7P2WMX9";

        // The client saw the impostor's certificate, so its proof binds to that
        // DER. Forwarded to the real host, it is wrong.
        let proof_from_client = pairing_proof(code, &impostor.cert_der);
        let window = PairingWindow::open(code.to_string(), now());
        let (outcome, _) = attempt_pairing(window, &proof_from_client, &real_host.cert_der, now());

        assert_ne!(
            outcome,
            PairingOutcome::Accepted,
            "a proof bound to the wrong certificate was accepted"
        );
    }

    #[test]
    fn the_right_code_over_the_right_certificate_pairs() {
        let host = HostIdentity::generate().unwrap();
        let code = "K7P2WMX9";
        let proof = pairing_proof(code, &host.cert_der);

        let window = PairingWindow::open(code.to_string(), now());
        let (outcome, remaining) = attempt_pairing(window, &proof, &host.cert_der, now());

        assert_eq!(outcome, PairingOutcome::Accepted);
        assert!(remaining.is_none(), "an accepted window should be consumed");
    }

    #[test]
    fn a_wrong_guess_leaves_the_window_open_with_one_fewer_try() {
        let host = HostIdentity::generate().unwrap();
        let window = PairingWindow::open("K7P2WMX9".to_string(), now());

        let (outcome, remaining) = attempt_pairing(window, "wrong", &host.cert_der, now());
        assert_eq!(
            outcome,
            PairingOutcome::Rejected {
                attempts_remaining: MAX_PAIRING_ATTEMPTS - 1
            }
        );
        assert_eq!(
            remaining.unwrap().attempts_remaining(),
            MAX_PAIRING_ATTEMPTS - 1
        );
    }

    /// Trap #3 from the indiAccounting handoff. If the exhausted window stayed
    /// open, the next caller — even one holding the right code — would be told
    /// to check their code rather than that nothing is pairing, and would go on
    /// retrying a window that can never accept them.
    #[test]
    fn the_last_wrong_guess_closes_the_window_rather_than_leaving_a_dead_one() {
        let host = HostIdentity::generate().unwrap();
        let mut window = Some(PairingWindow::open("K7P2WMX9".to_string(), now()));

        for _ in 0..MAX_PAIRING_ATTEMPTS - 1 {
            let (_, next) = attempt_pairing(window.take().unwrap(), "wrong", &host.cert_der, now());
            window = next;
            assert!(window.is_some());
        }

        let (outcome, next) = attempt_pairing(window.take().unwrap(), "wrong", &host.cert_der, now());
        assert_eq!(outcome, PairingOutcome::RejectedAndClosed);
        assert!(
            next.is_none(),
            "the exhausted window must be cleared, not left open and unusable"
        );
    }

    #[test]
    fn an_expired_window_refuses_even_the_right_code() {
        let host = HostIdentity::generate().unwrap();
        let code = "K7P2WMX9";
        let proof = pairing_proof(code, &host.cert_der);

        let opened = now();
        let window = PairingWindow::open(code.to_string(), opened);
        let later = opened + PAIRING_WINDOW + Duration::from_secs(1);

        let (outcome, next) = attempt_pairing(window, &proof, &host.cert_der, later);
        assert_eq!(outcome, PairingOutcome::RejectedAndClosed);
        assert!(next.is_none());
    }

    #[test]
    fn a_registered_device_can_present_its_token() {
        let db = Database::in_memory().unwrap();
        db.with_connection(|conn| {
            let (device, token) = register_device(conn, "Alex's laptop").unwrap();
            let found = device_for_token(conn, &token).unwrap().unwrap();
            assert_eq!(found.id, device.id);
            assert_eq!(found.label, "Alex's laptop");
            Ok(())
        })
        .unwrap();
    }

    /// A copy of the host database must not yield a working credential.
    #[test]
    fn only_the_hash_of_a_token_is_stored() {
        let db = Database::in_memory().unwrap();
        db.with_connection(|conn| {
            let (_, token) = register_device(conn, "Alex's laptop").unwrap();
            let stored: String = conn
                .query_row("SELECT token_hash FROM devices", [], |row| row.get(0))
                .unwrap();
            assert_ne!(stored, token);
            assert_eq!(stored, token_hash(&token));

            let any_plaintext: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM devices WHERE token_hash = ?1",
                    [&token],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(any_plaintext, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn an_unknown_token_matches_nothing() {
        let db = Database::in_memory().unwrap();
        db.with_connection(|conn| {
            register_device(conn, "Alex's laptop").unwrap();
            assert!(device_for_token(conn, &generate_device_token())
                .unwrap()
                .is_none());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_revoked_device_stops_being_found_by_its_token() {
        let db = Database::in_memory().unwrap();
        db.with_connection(|conn| {
            let (device, token) = register_device(conn, "Old laptop").unwrap();
            assert!(device_for_token(conn, &token).unwrap().is_some());

            revoke_device(conn, &device.id).unwrap();
            assert!(
                device_for_token(conn, &token).unwrap().is_none(),
                "a revoked machine should not authenticate again"
            );

            // It stays in the list so a person can see what they revoked.
            let listed = list_devices(conn).unwrap();
            assert_eq!(listed.len(), 1);
            assert!(listed[0].is_revoked);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn revoking_one_machine_leaves_the_others_working() {
        let db = Database::in_memory().unwrap();
        db.with_connection(|conn| {
            let (stolen, stolen_token) = register_device(conn, "Stolen laptop").unwrap();
            let (_, kept_token) = register_device(conn, "Kitchen tablet").unwrap();

            revoke_device(conn, &stolen.id).unwrap();

            assert!(device_for_token(conn, &stolen_token).unwrap().is_none());
            assert!(
                device_for_token(conn, &kept_token).unwrap().is_some(),
                "revoking one machine must not lock out the rest"
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn constant_time_comparison_still_compares_correctly() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(constant_time_eq(b"", b""));
    }
}
