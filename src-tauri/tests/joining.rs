//! A joining computer's whole life with a host, through the real commands.
//!
//! The host here is real and served over TLS on loopback; the joining side is a
//! real `AppState` with its own database on disk. Kept in its own file so it
//! runs in its own process: it points `XDG_DATA_HOME` at a scratch directory,
//! and environment variables are process-wide.

use std::sync::Arc;

use indibudget_lib::boundary::commands::build_registry;
use indibudget_lib::boundary::users::create_user;
use indibudget_lib::boundary::{Grants, Response, SharedState};
use indibudget_lib::commands::multiuser::{
    connect_on, disconnect_on, forget_on, invoke_on, pair_on, status_of, ConnectRequest,
    PairRequest,
};
use indibudget_lib::commands::AppState;
use indibudget_lib::database::{repository, Database};
use indibudget_lib::models::{Account, AccountType};
use indibudget_lib::net::credentials;
use indibudget_lib::net::host::{self, HostState, RunningHost};
use indibudget_lib::net::identity::HostIdentity;
use indibudget_lib::net::pairing::list_devices;
use serde_json::json;

struct Host {
    running: RunningHost,
    state: Arc<HostState>,
    db: Arc<Database>,
}

fn host_with(account: &str) -> Host {
    let db = Arc::new(Database::in_memory().unwrap());
    db.with_connection(|conn| {
        create_user(conn, "sam", "Sam", "Password1", true, &Grants::all(), None).unwrap();
        let a = Account::with_starting_balance(account.into(), AccountType::Checking, "10".parse().unwrap());
        repository::create_account(conn, &a)
    })
    .unwrap();
    let identity = db
        .with_connection(|conn| Ok(HostIdentity::load_or_create(conn).unwrap()))
        .unwrap();
    let state = Arc::new(HostState::new(
        Arc::clone(&db),
        Arc::new(build_registry()),
        identity,
        Arc::new(SharedState::new()),
    ));
    let running = host::start(Arc::clone(&state), "127.0.0.1:0".parse().unwrap()).unwrap();
    Host { running, state, db }
}

fn sign_in(client: &AppState, address: Option<String>) -> Result<(), String> {
    connect_on(
        client,
        ConnectRequest {
            login: "sam".into(),
            password: "Password1".into(),
            address,
        },
    )
    .map(|_| ())
}

fn account_names(client: &AppState) -> Result<Vec<String>, String> {
    match invoke_on(client, "get_accounts".into(), json!(null))? {
        Response::Ok { value } => Ok(value
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["name"].as_str().unwrap().to_string())
            .collect()),
        Response::Err { sentence, .. } => Err(sentence),
    }
}

fn fresh_app() -> AppState {
    let state = AppState::new();
    state.init_database().unwrap();
    state
}

#[test]
fn a_joining_computer_pairs_once_and_remembers_the_host() {
    let scratch = std::env::temp_dir().join(format!("indibudget-joining-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::env::set_var("XDG_DATA_HOME", &scratch);
    // Never the real keychain of whoever runs the tests.
    credentials::use_memory_store_for_tests();

    let mut host = host_with("Joint Checking");
    let address = host.running.addr().to_string();
    let devices = |h: &Host| h.db.with_connection(|c| Ok(list_devices(c).unwrap().len())).unwrap();

    // ---- pair, and the host is remembered at once
    let laptop = fresh_app();
    assert!(status_of(&laptop).saved_host.is_none());

    let code = host.state.open_pairing();
    let status = pair_on(
        &laptop,
        PairRequest { address: address.clone(), code, label: "Laptop".into() },
    )
    .expect("pairing");
    let saved = status.saved_host.expect("the host is remembered as soon as pairing succeeds");
    assert_eq!(saved.address, address);
    assert!(!saved.fingerprint_groups.is_empty());
    assert_eq!(devices(&host), 1);
    let stored = laptop
        .db
        .lock()
        .unwrap()
        .clone()
        .unwrap()
        .with_connection(|c| repository::get_setting(c, "joined_host"))
        .unwrap()
        .unwrap();
    assert!(
        !stored.contains("device_token"),
        "with a keychain available, the token stays out of the database: {stored}"
    );

    // ---- sign in; data now comes from the host, not this computer
    sign_in(&laptop, None).expect("sign in");
    assert!(status_of(&laptop).connected);
    assert_eq!(account_names(&laptop).unwrap(), vec!["Joint Checking"]);

    // ---- restart the app: only a sign-in is needed, and no new pairing
    drop(laptop);
    let laptop = fresh_app();
    let status = status_of(&laptop);
    assert!(!status.connected, "a restart starts disconnected");
    let saved = status.saved_host.expect("still remembered after a restart");
    assert_eq!(saved.last_login.as_deref(), Some("sam"), "the login name is remembered");
    sign_in(&laptop, None).expect("sign in after restart, without pairing again");
    assert_eq!(account_names(&laptop).unwrap(), vec!["Joint Checking"]);
    assert_eq!(devices(&host), 1, "a restart must not add another paired computer");

    // ---- pairing the same computer again replaces its entry on the host
    disconnect_on(&laptop);
    let code = host.state.open_pairing();
    pair_on(&laptop, PairRequest { address: address.clone(), code, label: "Laptop".into() })
        .expect("pairing again");
    assert_eq!(devices(&host), 1, "pairing again must not list this computer twice");
    sign_in(&laptop, None).expect("the new pairing works");

    // ---- a host remembered before the keychain was used moves its token there
    disconnect_on(&laptop);
    let local = laptop.db.lock().unwrap().clone().unwrap();
    let raw = local
        .with_connection(|c| repository::get_setting(c, "joined_host"))
        .unwrap()
        .unwrap();
    let mut old_style: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let account = format!("host-{}", old_style["fingerprint"].as_str().unwrap());
    let token = credentials::load(&account).expect("token in the keychain");
    credentials::forget(&account);
    old_style["device_token"] = json!(token);
    local
        .with_connection(|c| repository::set_setting(c, "joined_host", &old_style.to_string()))
        .unwrap();
    sign_in(&laptop, None).expect("an older saved host still signs in");
    let raw = local
        .with_connection(|c| repository::get_setting(c, "joined_host"))
        .unwrap()
        .unwrap();
    assert!(!raw.contains("device_token"), "moved out of the database: {raw}");
    assert_eq!(credentials::load(&account).as_deref(), Some(token.as_str()));

    // ---- a "new address" pointing at a different computer is refused
    disconnect_on(&laptop);
    let impostor = host_with("Somebody Else's Money");
    let err = sign_in(&laptop, Some(impostor.running.addr().to_string())).unwrap_err();
    assert!(!err.is_empty());
    assert_eq!(
        status_of(&laptop).saved_host.unwrap().address,
        address,
        "a failed attempt must not overwrite the remembered address"
    );
    sign_in(&laptop, None).expect("the real host still works");

    // ---- the host stops: no quiet fallback to this computer's own budget
    host.running.stop();
    let err = account_names(&laptop).expect_err("the host is gone");
    assert!(!err.is_empty());
    let status = status_of(&laptop);
    assert!(status.lost && !status.connected, "the dropped connection is reported");
    let err = account_names(&laptop).expect_err("still refused, not answered locally");
    assert!(err.contains("sign in again"), "{err}");

    // Disconnecting deliberately returns to this computer's own budget.
    disconnect_on(&laptop);
    assert!(!status_of(&laptop).lost);
    assert!(account_names(&laptop).unwrap().is_empty(), "this computer's own, empty budget");

    // ---- forgetting the host
    let status = forget_on(&laptop).unwrap();
    assert!(status.saved_host.is_none());
    let err = sign_in(&laptop, None).unwrap_err();
    assert!(err.contains("not been paired"), "{err}");

    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn stopping_hosting_disconnects_computers_that_already_joined() {
    // Uses only in-memory databases, so it does not touch XDG_DATA_HOME.
    let mut host = host_with("Joint Checking");
    let code = host.state.open_pairing();
    let mut pairing = indibudget_lib::net::client::Client::connect_for_pairing(host.running.addr()).unwrap();
    let fingerprint = pairing.host_fingerprint().unwrap();
    let token = pairing.pair(&code, "Laptop").unwrap();

    let mut client = indibudget_lib::net::client::Client::connect(host.running.addr(), fingerprint).unwrap();
    client.sign_in(&token, "sam", "Password1").unwrap();
    assert!(client
        .invoke(indibudget_lib::boundary::Request::new("get_accounts", json!(null)))
        .unwrap()
        .is_ok());

    host.running.stop();

    let after = client.invoke(indibudget_lib::boundary::Request::new("get_accounts", json!(null)));
    assert!(after.is_err(), "a joined computer kept reading after hosting stopped");
    assert!(client.is_broken());
}
