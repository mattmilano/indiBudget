//! Where a joining computer keeps the token from pairing.
//!
//! The device token is the credential that says "this computer was deliberately
//! added to that budget". It belongs in the operating system's own keychain —
//! Keychain on macOS, Credential Manager on Windows, the Secret Service (GNOME
//! Keyring, KWallet) on Linux — rather than in a database file that gets backed
//! up, copied and synced along with everything else.
//!
//! Not every computer has a keychain to offer: a Linux desktop without a
//! Secret Service, or a locked keyring nobody unlocks. There the token stays in
//! this computer's own database, as it did before, rather than leaving the
//! computer unable to join at all. Callers learn which happened from
//! [`store`]'s answer.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// The name entries are filed under in the keychain.
const SERVICE: &str = "indiBudget";

/// Set to anything to keep the keychain out of it, for a computer where the
/// keychain misbehaves.
const DISABLE_VAR: &str = "INDIBUDGET_NO_KEYCHAIN";

/// An in-memory stand-in for the keychain, for tests. `None` means the real
/// keychain is used.
fn memory() -> &'static Mutex<Option<HashMap<String, String>>> {
    static MEMORY: OnceLock<Mutex<Option<HashMap<String, String>>>> = OnceLock::new();
    MEMORY.get_or_init(|| Mutex::new(None))
}

fn with_memory<T>(f: impl FnOnce(&mut Option<HashMap<String, String>>) -> T) -> T {
    f(&mut memory().lock().unwrap_or_else(|p| p.into_inner()))
}

/// Keep secrets in memory instead of the real keychain, for the rest of this
/// process. Tests use this so they never write to the keychain of whoever runs
/// them.
#[doc(hidden)]
pub fn use_memory_store_for_tests() {
    with_memory(|m| {
        m.get_or_insert_with(HashMap::new);
    });
}

fn disabled() -> bool {
    std::env::var_os(DISABLE_VAR).is_some()
}

fn entry(account: &str) -> Option<keyring::Entry> {
    keyring::Entry::new(SERVICE, account).ok()
}

/// Put a secret in the keychain. `false` means there was no keychain to put it
/// in, and the caller should keep it somewhere else.
pub fn store(account: &str, secret: &str) -> bool {
    if let Some(stored) = with_memory(|m| {
        m.as_mut().map(|m| {
            m.insert(account.to_string(), secret.to_string());
            true
        })
    }) {
        return stored;
    }
    if disabled() {
        return false;
    }
    let Some(entry) = entry(account) else {
        return false;
    };
    // Read back what was written: some Secret Service setups accept a write
    // and keep nothing, and a token thought safe but actually lost would mean
    // pairing again with no explanation.
    entry.set_password(secret).is_ok() && entry.get_password().ok().as_deref() == Some(secret)
}

/// A secret from the keychain, if it holds one under that name.
pub fn load(account: &str) -> Option<String> {
    if let Some(found) = with_memory(|m| m.as_ref().map(|m| m.get(account).cloned())) {
        return found;
    }
    if disabled() {
        return None;
    }
    entry(account)?.get_password().ok()
}

/// Remove a secret. Quietly does nothing if there is none.
pub fn forget(account: &str) {
    if with_memory(|m| m.as_mut().map(|m| m.remove(account)).is_some()) {
        return;
    }
    if disabled() {
        return;
    }
    if let Some(entry) = entry(account) {
        let _ = entry.delete_credential();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_memory_store_keeps_and_forgets() {
        use_memory_store_for_tests();
        assert!(store("host-test", "token-1"));
        assert_eq!(load("host-test").as_deref(), Some("token-1"));
        assert!(store("host-test", "token-2"), "storing again replaces");
        assert_eq!(load("host-test").as_deref(), Some("token-2"));
        forget("host-test");
        assert_eq!(load("host-test"), None);
        forget("host-test"); // forgetting twice is harmless
    }
}
