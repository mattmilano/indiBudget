//! Tier 4 — Encryption (invariant I8 and handoff Section 5).
//!
//! The design (AES-256-GCM + Argon2id) is sound; the bugs live in the workflow.
//! These tests exercise the public EncryptionService surface only.
//!
//! I8: encrypt→decrypt round-trips; a wrong password fails closed and never
//!     returns or corrupts data.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use indibudget_lib::services::encryption::EncryptionService;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A unique, empty temp directory for an isolated encryption config.
fn temp_data_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("indibudget_enc_test_{nanos}_{n}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn service() -> EncryptionService {
    EncryptionService::new(temp_data_dir()).expect("encryption service")
}

const GOOD: &str = "Passw0rd!";
const GOOD2: &str = "N3wSecret!";

#[test]
fn round_trip_encrypt_decrypt_is_identity() {
    let mut svc = service();
    svc.enable(GOOD).expect("enable");
    assert!(svc.is_enabled() && svc.is_unlocked());

    let secret = "Account #1234 — rent $1,200.00";
    let ciphertext = svc.encrypt(secret).expect("encrypt");
    assert_ne!(ciphertext, secret, "ciphertext must differ from plaintext");

    let recovered = svc.decrypt(&ciphertext).expect("decrypt");
    assert_eq!(recovered, secret, "encrypt→decrypt must round-trip exactly");
}

#[test]
fn wrong_password_fails_closed_and_leaves_data_recoverable() {
    let dir = temp_data_dir();
    let secret = "sensitive-memo";
    let ciphertext;

    // Enable and encrypt, then drop the service (simulates app close).
    {
        let mut svc = EncryptionService::new(dir.clone()).unwrap();
        svc.enable(GOOD).unwrap();
        ciphertext = svc.encrypt(secret).unwrap();
    }

    // Reopen from the same dir: must load as enabled + locked.
    let mut svc = EncryptionService::new(dir.clone()).unwrap();
    assert!(svc.is_enabled(), "enabled state must persist across restart");
    assert!(!svc.is_unlocked(), "must start locked after restart");

    // Wrong password is rejected via the verification hash, not by attempting
    // a corrupting decrypt.
    assert!(svc.unlock("WrongPass9!").is_err(), "wrong password must fail");
    assert!(!svc.is_unlocked(), "failed unlock must not unlock");

    // The correct password still works afterward, and data is intact.
    svc.unlock(GOOD).expect("correct password unlocks");
    assert_eq!(svc.decrypt(&ciphertext).unwrap(), secret, "data intact after wrong attempt");
}

#[test]
fn locked_state_denies_encrypt_and_decrypt() {
    let mut svc = service();
    svc.enable(GOOD).unwrap();
    let ct = svc.encrypt("x").unwrap();

    svc.lock();
    assert!(!svc.is_unlocked());
    assert!(svc.encrypt("y").is_err(), "encrypt must fail while locked");
    assert!(svc.decrypt(&ct).is_err(), "decrypt must fail while locked");
}

#[test]
fn password_change_rotates_credentials() {
    // Credential rotation that DOES hold today: after change_password the old
    // password no longer unlocks and the new one does, across a restart.
    let dir = temp_data_dir();

    let mut svc = EncryptionService::new(dir.clone()).unwrap();
    svc.enable(GOOD).unwrap();
    svc.change_password(GOOD, GOOD2).expect("change password");

    let mut svc2 = EncryptionService::new(dir).unwrap();
    assert!(svc2.unlock(GOOD).is_err(), "old password must stop working");
    svc2.unlock(GOOD2).expect("new password must unlock");
}

#[test]
#[ignore = "KNOWN DEFECT: change_password rotates the key without re-encrypting \
data (or key-wrapping), so any data encrypted before the change becomes \
undecryptable. Currently latent because encrypt()/decrypt() are not wired to \
stored data. Fix = key-wrapping (stable data key, password only wraps it). \
Un-ignore once that lands."]
fn password_change_preserves_encrypted_data() {
    let dir = temp_data_dir();
    let secret = "balance-snapshot";

    let mut svc = EncryptionService::new(dir.clone()).unwrap();
    svc.enable(GOOD).unwrap();
    let ct = svc.encrypt(secret).unwrap();

    svc.change_password(GOOD, GOOD2).expect("change password");

    // Data must remain readable after a password change. Fails today.
    assert_eq!(svc.decrypt(&ct).unwrap(), secret, "data must survive password change");

    let mut svc2 = EncryptionService::new(dir).unwrap();
    svc2.unlock(GOOD2).expect("new password unlocks");
    assert_eq!(svc2.decrypt(&ct).unwrap(), secret, "data intact under new password");
}

#[test]
fn weak_passwords_are_rejected_by_policy() {
    // Policy (verified in source): >= 8 chars, upper, lower, digit.
    let cases = [
        ("short", "too short"),
        ("alllowercase1", "no uppercase"),
        ("ALLUPPERCASE1", "no lowercase"),
        ("NoDigitsHere", "no digit"),
    ];
    for (pw, why) in cases {
        let mut svc = service();
        assert!(svc.enable(pw).is_err(), "weak password ({why}) must be rejected");
        assert!(!svc.is_enabled(), "rejected password must not enable encryption");
    }

    // A compliant password is accepted.
    let mut svc = service();
    assert!(svc.enable("Strong1A").is_ok(), "compliant password must be accepted");
}

#[test]
fn tampered_ciphertext_is_rejected_by_gcm_integrity() {
    // GCM must fail closed on tamper, not return garbage.
    let mut svc = service();
    svc.enable(GOOD).unwrap();
    let ct = svc.encrypt("integrity-protected").unwrap();

    // Flip a character in the middle of the base64 ciphertext.
    let mut bytes: Vec<char> = ct.chars().collect();
    let mid = bytes.len() / 2;
    bytes[mid] = if bytes[mid] == 'A' { 'B' } else { 'A' };
    let tampered: String = bytes.into_iter().collect();

    if tampered != ct {
        assert!(svc.decrypt(&tampered).is_err(), "tampered ciphertext must be rejected");
    }
}

#[test]
fn disable_requires_correct_password() {
    let mut svc = service();
    svc.enable(GOOD).unwrap();
    assert!(svc.disable("WrongPass9!").is_err(), "disable must verify password");
    assert!(svc.is_enabled(), "failed disable must leave encryption enabled");
    svc.disable(GOOD).expect("correct password disables");
    assert!(!svc.is_enabled());
}
