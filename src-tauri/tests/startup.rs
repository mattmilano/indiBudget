//! The real startup sequence, end to end through the real command routing.
//!
//! Every call from the frontend goes through `boundary_invoke`. A command the
//! registry does not know comes back as "unknown", and `lib/rpc.ts` then runs it
//! directly as a host-only command. This test plays the frontend's part exactly
//! that way, against a real `AppState` and a real database on disk.
//!
//! It exists because the routing once checked for an open database *before*
//! checking the registry. `init_app` — the command that opens the database — is
//! host-only, so it was refused for want of the database it was meant to open.
//! Startup failed without a word, and the first symptom anyone saw was the user
//! agreement's "I Agree" button doing nothing. Nothing tested that path: the
//! screenshot checks mocked `boundary_invoke` out entirely.
//!
//! Kept in its own file so it runs in its own process: it points
//! `XDG_DATA_HOME` at a scratch directory, and environment variables are
//! process-wide.

use indibudget_lib::boundary::{BoundaryError, Response};
use indibudget_lib::commands::multiuser::invoke_on;
use indibudget_lib::commands::AppState;
use indibudget_lib::database::repository;
use serde_json::{json, Value};

/// What `lib/rpc.ts` does with the answer to `boundary_invoke`.
enum Routed {
    /// Dispatched through the registry.
    Handled(Response),
    /// Not registered: the frontend now runs it as a host-only command.
    HostOnly,
}

fn route(state: &AppState, command: &str, args: Value) -> Routed {
    let response = invoke_on(state, command.to_string(), args)
        .unwrap_or_else(|e| panic!("`{command}` failed inside boundary_invoke itself: {e}"));
    match response {
        Response::Err {
            error: BoundaryError::UnknownCommand { .. },
            ..
        } => Routed::HostOnly,
        other => Routed::Handled(other),
    }
}

fn setting(state: &AppState, key: &str) -> Option<String> {
    let db = state.db.lock().unwrap().clone().expect("database open");
    db.with_connection(|conn| repository::get_setting(conn, key))
        .unwrap()
}

fn set_setting(state: &AppState, key: &str, value: &str) {
    let db = state.db.lock().unwrap().clone().expect("database open");
    db.with_connection(|conn| repository::set_setting(conn, key, value))
        .unwrap();
}

#[test]
fn first_run_opens_the_database_and_the_agreement_can_be_accepted() {
    let scratch = std::env::temp_dir().join(format!("indibudget-startup-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::env::set_var("XDG_DATA_HOME", &scratch);

    let state = AppState::new();

    // Before the database is open, a command that needs it says so plainly,
    // rather than failing in a way the frontend cannot tell apart from success.
    let err = invoke_on(&state, "get_accounts".into(), json!(null)).unwrap_err();
    assert!(err.contains("not initialized"), "{err}");

    // main.ts: initApp(). This is the call that used to fail.
    match route(&state, "init_app", json!(null)) {
        Routed::HostOnly => state.init_database().expect("database opens"),
        Routed::Handled(r) => panic!("init_app should be host-only, got {r:?}"),
    }
    assert!(
        scratch.join("indibudget/indibudget.db").exists(),
        "the database file was not created"
    );

    // App.vue: has the agreement been accepted? On first run, no.
    match route(&state, "get_setting", json!({ "key": "user_agreement_accepted" })) {
        Routed::HostOnly => assert_eq!(setting(&state, "user_agreement_accepted"), None),
        Routed::Handled(r) => panic!("get_setting should be host-only, got {r:?}"),
    }

    // UserAgreementModal: "I Agree". This is the click that did nothing.
    for (key, value) in [
        ("user_agreement_accepted", "true"),
        ("user_agreement_version", "1.1"),
        ("user_agreement_accepted_at", "2026-10-07T00:00:00Z"),
    ] {
        match route(&state, "set_setting", json!({ "key": key, "value": value })) {
            Routed::HostOnly => set_setting(&state, key, value),
            Routed::Handled(r) => panic!("set_setting should be host-only, got {r:?}"),
        }
    }

    // Next launch reads it back, and the app opens instead of the agreement.
    assert_eq!(setting(&state, "user_agreement_accepted").as_deref(), Some("true"));
    assert_eq!(setting(&state, "user_agreement_version").as_deref(), Some("1.1"));

    // And the ordinary screens now work through the registry.
    match route(&state, "get_accounts", json!(null)) {
        Routed::Handled(Response::Ok { .. }) => {}
        Routed::Handled(r) => panic!("get_accounts was refused: {r:?}"),
        Routed::HostOnly => panic!("get_accounts should be registered"),
    }

    let _ = std::fs::remove_dir_all(&scratch);
}
