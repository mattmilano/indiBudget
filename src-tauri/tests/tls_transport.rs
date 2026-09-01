//! Multi-user phase 3: the transport, end to end over real TLS.
//!
//! The handoff was emphatic that this test be written at the end of phase 3
//! rather than at the end of the project, because everything built afterwards
//! inherits whatever this path actually does. So: two real endpoints, a real
//! handshake, real framing, and the boundary on the far side.

mod common;

use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;

use indibudget_lib::boundary::registry::{encode, BoundaryCtx, Registry};
use indibudget_lib::boundary::users::create_user;
use indibudget_lib::boundary::{Access, Area, BoundaryError, Grants, Request, Required, Response};
use indibudget_lib::database::{repository, Database};
use indibudget_lib::models::*;
use indibudget_lib::net::client::Client;
use indibudget_lib::net::host::{self, HostState, RunningHost};
use indibudget_lib::net::identity::{Fingerprint, HostIdentity};
use indibudget_lib::net::pairing::{list_devices, revoke_device};

// ---------------------------------------------------------------- handlers

fn h_get_accounts(ctx: &BoundaryCtx, _args: Value) -> Result<Value, BoundaryError> {
    let accounts = ctx
        .db
        .with_connection(|conn| repository::get_all_accounts(conn))
        .map_err(|e| BoundaryError::internal(e.to_string()))?;
    encode(accounts)
}

fn h_create_category(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BoundaryError::invalid("A category name is required."))?;
    let category = Category::new(name.to_string(), CategoryType::Expense, "#888888".into());
    ctx.db
        .with_connection(|conn| repository::create_category(conn, &category))
        .map_err(|e| BoundaryError::internal(e.to_string()))?;
    encode(category)
}

fn test_registry() -> Registry {
    let mut registry = Registry::new();
    registry.register("get_accounts", Required::read(Area::Money), h_get_accounts);
    registry.register(
        "create_category",
        Required::write(Area::Structure),
        h_create_category,
    );
    registry
}

// ------------------------------------------------------------------ set-up

struct Fixture {
    host: RunningHost,
    state: Arc<HostState>,
    db: Arc<Database>,
}

impl Fixture {
    fn addr(&self) -> SocketAddr {
        self.host.addr()
    }

    fn fingerprint(&self) -> Fingerprint {
        self.state.identity.fingerprint()
    }
}

/// A host with one owner, one limited member, and one account to read.
fn hosted() -> Fixture {
    let db = Arc::new(Database::in_memory().expect("db"));

    db.with_connection(|conn| {
        create_user(conn, "sam", "Sam", "Password1", true, &Grants::all(), None).unwrap();
        create_user(
            conn,
            "alex",
            "Alex",
            "Password1",
            false,
            // Deliberately read-only on Money and nothing on Structure.
            &Grants::none().with(Area::Money, Access::Read),
            None,
        )
        .unwrap();
        Ok(())
    })
    .unwrap();

    let account = Account::with_starting_balance(
        "Joint Checking".into(),
        AccountType::Checking,
        "1250.00".parse().unwrap(),
    );
    db.with_connection(|conn| repository::create_account(conn, &account))
        .unwrap();

    let identity = db
        .with_connection(|conn| Ok(HostIdentity::load_or_create(conn).unwrap()))
        .unwrap();

    let state = Arc::new(HostState::new(
        Arc::clone(&db),
        Arc::new(test_registry()),
        identity,
    ));
    let host = host::start(Arc::clone(&state), "127.0.0.1:0".parse().unwrap()).expect("host starts");

    Fixture { host, state, db }
}

/// Pair a fresh machine and return its device token.
fn pair_a_machine(fixture: &Fixture, label: &str) -> (String, Fingerprint) {
    let code = fixture.state.open_pairing();
    let mut client = Client::connect_for_pairing(fixture.addr()).expect("pairing connection");
    let fingerprint = client.host_fingerprint().expect("a fingerprint was observed");
    let token = client.pair(&code, label).expect("pairing should succeed");
    (token, fingerprint)
}

// ------------------------------------------------------------------- tests

#[test]
fn a_machine_pairs_signs_in_and_reads_over_tls() {
    let fixture = hosted();
    let (token, fingerprint) = pair_a_machine(&fixture, "Alex's laptop");

    assert_eq!(
        fingerprint,
        fixture.fingerprint(),
        "the fingerprint learned while pairing should be the host's own"
    );

    let mut client = Client::connect(fixture.addr(), fingerprint).expect("pinned connection");
    let session = client.sign_in(&token, "sam", "Password1").expect("sign-in");
    assert_eq!(session.display_name, "Sam");
    assert!(session.is_owner);

    let response = client
        .invoke(Request::new("get_accounts", json!({})))
        .expect("the call should reach the host");

    match response {
        Response::Ok { value } => {
            let accounts = value.as_array().expect("an array of accounts");
            assert_eq!(accounts.len(), 1);
            assert_eq!(accounts[0]["name"], "Joint Checking");
        }
        Response::Err { sentence, .. } => panic!("unexpected refusal: {sentence}"),
    }
}

#[test]
fn the_pin_is_enforced_against_a_different_host() {
    let first = hosted();
    let second = hosted();

    let (_, first_fingerprint) = pair_a_machine(&first, "Alex's laptop");
    assert_ne!(first_fingerprint, second.fingerprint());

    // Connecting to the second host while pinned to the first must fail during
    // the handshake, before any application byte is exchanged.
    let outcome = Client::connect(second.addr(), first_fingerprint)
        .and_then(|mut c| c.invoke(Request::new("get_accounts", json!({}))));

    assert!(
        outcome.is_err(),
        "a host with a different certificate was accepted under the wrong pin"
    );
}

#[test]
fn a_wrong_pairing_code_is_refused() {
    let fixture = hosted();
    let _real_code = fixture.state.open_pairing();

    let mut client = Client::connect_for_pairing(fixture.addr()).unwrap();
    let err = client.pair("WRONGCODE", "Impostor").unwrap_err();
    assert!(
        err.sentence().contains("did not match"),
        "sentence: {}",
        err.sentence()
    );
}

/// Trap #3, over the wire this time: after the last wrong guess the window
/// must be gone, and the next caller told nothing is pairing rather than sent
/// round a loop checking a code that can never work.
#[test]
fn the_exhausted_pairing_window_reports_that_nothing_is_pairing() {
    let fixture = hosted();
    let code = fixture.state.open_pairing();

    for _ in 0..5 {
        let mut client = Client::connect_for_pairing(fixture.addr()).unwrap();
        let _ = client.pair("WRONGCODE", "Impostor");
    }

    assert!(
        !fixture.state.is_pairing(),
        "the window should have closed itself"
    );

    // Even the correct code now gets told there is nothing to pair with.
    let mut client = Client::connect_for_pairing(fixture.addr()).unwrap();
    let err = client.pair(&code, "Alex's laptop").unwrap_err();
    assert!(
        err.sentence().contains("not accepting"),
        "the right code should be told pairing is closed, not to check the code: {}",
        err.sentence()
    );
}

#[test]
fn pairing_is_refused_when_no_window_is_open() {
    let fixture = hosted();
    // Never opened.
    let mut client = Client::connect_for_pairing(fixture.addr()).unwrap();
    let err = client.pair("ANYTHING", "Impostor").unwrap_err();
    assert!(err.sentence().contains("not accepting"), "{}", err.sentence());
}

#[test]
fn commands_are_refused_before_sign_in() {
    let fixture = hosted();
    let (_, fingerprint) = pair_a_machine(&fixture, "Alex's laptop");

    let mut client = Client::connect(fixture.addr(), fingerprint).unwrap();
    let err = client
        .invoke(Request::new("get_accounts", json!({})))
        .unwrap_err();
    assert!(err.sentence().contains("sign in"), "{}", err.sentence());
}

#[test]
fn a_wrong_password_is_refused_with_the_same_sentence_as_an_unknown_login() {
    let fixture = hosted();
    let (token, fingerprint) = pair_a_machine(&fixture, "Alex's laptop");

    let mut client = Client::connect(fixture.addr(), fingerprint).unwrap();
    let wrong_password = client.sign_in(&token, "sam", "Wrong1234").unwrap_err();

    let mut client = Client::connect(fixture.addr(), fingerprint).unwrap();
    let unknown_login = client.sign_in(&token, "nobody", "Wrong1234").unwrap_err();

    assert_eq!(
        wrong_password.sentence(),
        unknown_login.sentence(),
        "the two failures must be indistinguishable over the wire"
    );
}

/// Grants are re-checked at the boundary on the far side, so a remote caller
/// cannot reach what the same person could not reach locally.
#[test]
fn a_members_grants_are_enforced_over_the_wire() {
    let fixture = hosted();
    let (token, fingerprint) = pair_a_machine(&fixture, "Alex's laptop");

    let mut client = Client::connect(fixture.addr(), fingerprint).unwrap();
    client.sign_in(&token, "alex", "Password1").unwrap();

    // Alex has Money: Read.
    let allowed = client.invoke(Request::new("get_accounts", json!({}))).unwrap();
    assert!(allowed.is_ok(), "a read grant should have been honoured");

    // Alex has nothing on Structure.
    let refused = client
        .invoke(Request::new("create_category", json!({ "name": "Sneaky" })))
        .unwrap();
    match refused {
        Response::Err { sentence, .. } => {
            assert!(sentence.contains("Alex"), "refusal should name them: {sentence}");
            assert!(sentence.contains("Structure"), "{sentence}");
        }
        Response::Ok { .. } => panic!("a member without a Structure grant created a category"),
    }

    // And nothing was written.
    let count: i64 = fixture
        .db
        .with_connection(|conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM categories WHERE name = 'Sneaky'",
                [],
                |row| row.get(0),
            )?;
            Ok(n)
        })
        .unwrap();
    assert_eq!(count, 0, "a refused write must not have touched the data");
}

#[test]
fn an_owner_may_do_what_the_member_could_not() {
    let fixture = hosted();
    let (token, fingerprint) = pair_a_machine(&fixture, "Sam's desktop");

    let mut client = Client::connect(fixture.addr(), fingerprint).unwrap();
    client.sign_in(&token, "sam", "Password1").unwrap();

    let response = client
        .invoke(Request::new("create_category", json!({ "name": "Holiday" })))
        .unwrap();
    assert!(response.is_ok(), "the owner should have been allowed");
}

/// A command absent from the registry does not exist remotely — the safe
/// default that keeps dialogs, local backups, and encryption host-only.
#[test]
fn an_unregistered_command_does_not_exist_over_the_wire() {
    let fixture = hosted();
    let (token, fingerprint) = pair_a_machine(&fixture, "Alex's laptop");

    let mut client = Client::connect(fixture.addr(), fingerprint).unwrap();
    client.sign_in(&token, "sam", "Password1").unwrap();

    let response = client
        .invoke(Request::new("export_backup_to_file", json!({ "path": "/tmp/x" })))
        .unwrap();

    match response {
        Response::Err { sentence, .. } => assert!(
            sentence.contains("hosting"),
            "should say it runs at the host: {sentence}"
        ),
        Response::Ok { .. } => panic!("an unregistered command ran remotely"),
    }
}

#[test]
fn a_revoked_machine_cannot_sign_in_again() {
    let fixture = hosted();
    let (token, fingerprint) = pair_a_machine(&fixture, "Old laptop");

    // It works to begin with.
    let mut client = Client::connect(fixture.addr(), fingerprint).unwrap();
    client.sign_in(&token, "sam", "Password1").unwrap();

    let device_id = fixture
        .db
        .with_connection(|conn| Ok(list_devices(conn).unwrap()[0].id.clone()))
        .unwrap();
    fixture
        .db
        .with_connection(|conn| Ok(revoke_device(conn, &device_id).unwrap()))
        .unwrap();

    // The next connection is refused. Revocation applies at the next
    // connection, not the next instant — the lever is for the machine.
    let mut client = Client::connect(fixture.addr(), fingerprint).unwrap();
    let err = client.sign_in(&token, "sam", "Password1").unwrap_err();
    assert!(
        err.sentence().contains("paired again"),
        "{}",
        err.sentence()
    );
}

#[test]
fn repeated_sign_in_failures_are_throttled() {
    let fixture = hosted();
    let (token, fingerprint) = pair_a_machine(&fixture, "Alex's laptop");

    let mut last = None;
    for _ in 0..6 {
        let mut client = Client::connect(fixture.addr(), fingerprint).unwrap();
        last = Some(client.sign_in(&token, "sam", "Wrong1234").unwrap_err());
    }

    let sentence = last.unwrap().sentence();
    assert!(
        sentence.contains("Too many"),
        "repeated failures should start being held off: {sentence}"
    );
}

#[test]
fn throttling_one_login_does_not_block_another_person() {
    let fixture = hosted();
    let (token, fingerprint) = pair_a_machine(&fixture, "Shared tablet");

    for _ in 0..6 {
        let mut client = Client::connect(fixture.addr(), fingerprint).unwrap();
        let _ = client.sign_in(&token, "sam", "Wrong1234");
    }

    let mut client = Client::connect(fixture.addr(), fingerprint).unwrap();
    let session = client
        .sign_in(&token, "alex", "Password1")
        .expect("another person should still be able to sign in");
    assert_eq!(session.display_name, "Alex");
}

#[test]
fn two_machines_can_be_paired_and_used_independently() {
    let fixture = hosted();
    let (sam_token, fingerprint) = pair_a_machine(&fixture, "Sam's desktop");
    let (alex_token, _) = pair_a_machine(&fixture, "Alex's laptop");
    assert_ne!(sam_token, alex_token, "each machine gets its own token");

    let mut sam = Client::connect(fixture.addr(), fingerprint).unwrap();
    sam.sign_in(&sam_token, "sam", "Password1").unwrap();

    let mut alex = Client::connect(fixture.addr(), fingerprint).unwrap();
    alex.sign_in(&alex_token, "alex", "Password1").unwrap();

    // Both connections stay usable at the same time.
    assert!(sam.invoke(Request::new("get_accounts", json!({}))).unwrap().is_ok());
    assert!(alex.invoke(Request::new("get_accounts", json!({}))).unwrap().is_ok());
    assert!(sam
        .invoke(Request::new("create_category", json!({ "name": "Garden" })))
        .unwrap()
        .is_ok());

    let devices = fixture
        .db
        .with_connection(|conn| Ok(list_devices(conn).unwrap()))
        .unwrap();
    assert_eq!(devices.len(), 2);
}

#[test]
fn a_write_made_remotely_is_visible_to_the_host() {
    let fixture = hosted();
    let (token, fingerprint) = pair_a_machine(&fixture, "Sam's desktop");

    let mut client = Client::connect(fixture.addr(), fingerprint).unwrap();
    client.sign_in(&token, "sam", "Password1").unwrap();
    client
        .invoke(Request::new("create_category", json!({ "name": "Allotment" })))
        .unwrap();

    let found: i64 = fixture
        .db
        .with_connection(|conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM categories WHERE name = 'Allotment'",
                [],
                |row| row.get(0),
            )?;
            Ok(n)
        })
        .unwrap();
    assert_eq!(found, 1, "a remote write should have landed in the one database");
}
