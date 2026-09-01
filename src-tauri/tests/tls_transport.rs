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
use indibudget_lib::boundary::news::{CatchUp, Notice};
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

    // The shape every swept write will take: announce only after it landed.
    ctx.shared.news.publish(Notice::RecordChanged {
        area: Area::Structure,
        record_kind: "category".into(),
        record_id: category.id.clone(),
    });
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
    indibudget_lib::boundary::leases::register(&mut registry);
    indibudget_lib::boundary::news::register(&mut registry);
    indibudget_lib::boundary::maintenance::register(&mut registry);
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
        create_user(
            conn,
            "jo",
            "Jo",
            "Password1",
            false,
            &Grants::none().with(Area::Planning, Access::Write),
            None,
        )
        .unwrap();
        // A second administrator, so "any administrator may reopen" is a claim
        // about two different people rather than one.
        create_user(conn, "pat", "Pat", "Password1", true, &Grants::all(), None).unwrap();
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

// ------------------------------------------------------- edit holds (phase 4)

fn lease(client: &mut Client, command: &str, kind: &str, id: &str) -> Response {
    client
        .invoke(Request::new(
            command,
            json!({ "kind": kind, "record_id": id }),
        ))
        .expect("the call should reach the host")
}

fn signed_in_client(fixture: &Fixture, login: &str) -> Client {
    let (token, fingerprint) = pair_a_machine(fixture, login);
    let mut client = Client::connect(fixture.addr(), fingerprint).unwrap();
    client.sign_in(&token, login, "Password1").unwrap();
    client
}

#[test]
fn a_second_person_opening_the_same_budget_is_told_who_has_it() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut jo = signed_in_client(&fixture, "jo");

    assert!(lease(&mut sam, "lease_acquire", "budget", "b1").is_ok());

    match lease(&mut jo, "lease_acquire", "budget", "b1") {
        Response::Err { sentence, .. } => {
            assert!(sentence.contains("Sam"), "should name the holder: {sentence}");
            assert!(sentence.contains("budget"), "{sentence}");
        }
        Response::Ok { .. } => panic!("two people took the same hold"),
    }
}

#[test]
fn releasing_a_hold_lets_the_next_person_in() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut jo = signed_in_client(&fixture, "jo");

    lease(&mut sam, "lease_acquire", "budget", "b1");
    assert!(lease(&mut jo, "lease_acquire", "budget", "b1").is_err_response());

    assert!(lease(&mut sam, "lease_release", "budget", "b1").is_ok());
    assert!(
        lease(&mut jo, "lease_acquire", "budget", "b1").is_ok(),
        "the record should be free once its holder let go"
    );
}

#[test]
fn holds_on_different_budgets_do_not_collide() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut jo = signed_in_client(&fixture, "jo");

    assert!(lease(&mut sam, "lease_acquire", "budget", "groceries").is_ok());
    assert!(
        lease(&mut jo, "lease_acquire", "budget", "fuel").is_ok(),
        "two people editing different budgets must not queue behind each other"
    );
}

/// The household case the split exists for: both partners logging receipts at
/// once must never wait on each other.
#[test]
fn transactions_have_no_hold_to_contend_over() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");

    // "transaction" is not a leasable kind, so the request cannot even be
    // expressed — the wire enum rejects it.
    match lease(&mut sam, "lease_acquire", "transaction", "t1") {
        Response::Err { sentence, .. } => assert!(
            sentence.contains("not in the expected form"),
            "{sentence}"
        ),
        Response::Ok { .. } => panic!("a transaction was leased"),
    }
}

#[test]
fn someone_without_the_grant_cannot_park_a_hold() {
    let fixture = hosted();
    // Alex has Money: Read and nothing on Planning.
    let mut alex = signed_in_client(&fixture, "alex");

    match lease(&mut alex, "lease_acquire", "budget", "b1") {
        Response::Err { sentence, .. } => {
            assert!(sentence.contains("Alex"), "{sentence}");
            assert!(sentence.contains("Planning"), "{sentence}");
        }
        Response::Ok { .. } => {
            panic!("a read-only member parked a hold on a budget they cannot edit")
        }
    }
}

#[test]
fn a_holder_can_renew_and_keep_it() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut jo = signed_in_client(&fixture, "jo");

    lease(&mut sam, "lease_acquire", "budget", "b1");
    assert!(lease(&mut sam, "lease_renew", "budget", "b1").is_ok());
    assert!(lease(&mut jo, "lease_acquire", "budget", "b1").is_err_response());
}

#[test]
fn one_person_cannot_release_anothers_hold_over_the_wire() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut jo = signed_in_client(&fixture, "jo");

    lease(&mut sam, "lease_acquire", "budget", "b1");
    lease(&mut jo, "lease_release", "budget", "b1");

    assert!(
        lease(&mut jo, "lease_acquire", "budget", "b1").is_err_response(),
        "Jo released a hold belonging to Sam"
    );
}

#[test]
fn holders_can_be_listed_for_badges() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut jo = signed_in_client(&fixture, "jo");

    lease(&mut sam, "lease_acquire", "budget", "groceries");

    let response = jo
        .invoke(Request::new("lease_holders", json!({ "kind": "budget" })))
        .unwrap();
    match response {
        Response::Ok { value } => {
            let held = value.as_array().unwrap();
            assert_eq!(held.len(), 1);
            assert_eq!(held[0]["record_id"], "groceries");
            assert_eq!(held[0]["holder"], "Sam");
        }
        Response::Err { sentence, .. } => panic!("unexpected refusal: {sentence}"),
    }
}

/// A laptop that closed its lid should not hold the grocery budget for the
/// rest of the lease.
#[test]
fn a_dropped_connection_gives_up_its_holds() {
    let fixture = hosted();
    let mut jo = signed_in_client(&fixture, "jo");

    {
        let mut sam = signed_in_client(&fixture, "sam");
        lease(&mut sam, "lease_acquire", "budget", "b1");
        assert!(lease(&mut jo, "lease_acquire", "budget", "b1").is_err_response());
    } // Sam's client drops here, closing the connection.

    // Give the host's connection thread a moment to notice and clean up.
    for _ in 0..50 {
        if lease(&mut jo, "lease_acquire", "budget", "b1").is_ok() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("a closed connection left its holds behind");
}

/// Small helper so the assertions above read as intent rather than matching.
trait ResponseExt {
    fn is_err_response(&self) -> bool;
}

impl ResponseExt for Response {
    fn is_err_response(&self) -> bool {
        !self.is_ok()
    }
}

// ------------------------------------------------------------ news (phase 5)

fn catch_up(client: &mut Client, mark: Option<&serde_json::Value>) -> CatchUp {
    let args = match mark {
        Some(m) => json!({ "mark": m }),
        None => json!({ "mark": null }),
    };
    let response = client
        .invoke(Request::new("news_catch_up", args))
        .expect("the call should reach the host");
    match response {
        Response::Ok { value } => serde_json::from_value(value).expect("a catch-up result"),
        Response::Err { sentence, .. } => panic!("unexpected refusal: {sentence}"),
    }
}

fn mark_value(result: &CatchUp) -> serde_json::Value {
    let mark = match result {
        CatchUp::Notices { mark, .. } | CatchUp::StartOver { mark } => mark,
    };
    serde_json::to_value(mark).unwrap()
}

fn heard(result: &CatchUp) -> &[Notice] {
    match result {
        CatchUp::Notices { notices, .. } => notices,
        CatchUp::StartOver { .. } => panic!("expected notices, got start_over"),
    }
}

#[test]
fn one_persons_write_is_heard_by_another() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut jo = signed_in_client(&fixture, "jo");

    // Jo has Planning only, so give them a grant that can hear Structure by
    // using the owner as the listener instead.
    let mut listener = signed_in_client(&fixture, "sam");
    let start = mark_value(&catch_up(&mut listener, None));

    sam.invoke(Request::new("create_category", json!({ "name": "Allotment" })))
        .unwrap();

    let result = catch_up(&mut listener, Some(&start));
    let notices = heard(&result);
    assert_eq!(notices.len(), 1, "the write should have been announced");
    match &notices[0] {
        Notice::RecordChanged { record_kind, .. } => assert_eq!(record_kind, "category"),
        other => panic!("expected RecordChanged, got {other:?}"),
    }

    // And Jo, who cannot read Structure, hears nothing about it.
    let jo_start = mark_value(&catch_up(&mut jo, None));
    sam.invoke(Request::new("create_category", json!({ "name": "Shed" })))
        .unwrap();
    let jo_result = catch_up(&mut jo, Some(&jo_start));
    assert!(
        heard(&jo_result).is_empty(),
        "Jo has no Structure grant and should not hear about categories"
    );
}

#[test]
fn a_hold_is_announced_with_the_holders_name() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut jo = signed_in_client(&fixture, "jo");

    let start = mark_value(&catch_up(&mut jo, None));
    lease(&mut sam, "lease_acquire", "budget", "groceries");

    let result = catch_up(&mut jo, Some(&start));
    let notices = heard(&result);
    assert_eq!(notices.len(), 1);
    match &notices[0] {
        Notice::RecordBusy {
            holder, record_id, ..
        } => {
            assert_eq!(holder, "Sam");
            assert_eq!(record_id, "groceries");
        }
        other => panic!("expected RecordBusy, got {other:?}"),
    }
}

#[test]
fn letting_go_is_announced_so_the_badge_comes_down() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut jo = signed_in_client(&fixture, "jo");

    lease(&mut sam, "lease_acquire", "budget", "groceries");
    let start = mark_value(&catch_up(&mut jo, None));

    lease(&mut sam, "lease_release", "budget", "groceries");

    let result = catch_up(&mut jo, Some(&start));
    let notices = heard(&result);
    assert_eq!(notices.len(), 1);
    assert!(matches!(notices[0], Notice::RecordFreed { .. }));
}

/// A beat every twenty seconds per open editor would bury the log.
#[test]
fn renewals_are_silent() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut jo = signed_in_client(&fixture, "jo");

    lease(&mut sam, "lease_acquire", "budget", "groceries");
    let start = mark_value(&catch_up(&mut jo, None));

    for _ in 0..5 {
        lease(&mut sam, "lease_renew", "budget", "groceries");
    }

    let result = catch_up(&mut jo, Some(&start));
    assert!(
        heard(&result).is_empty(),
        "five heartbeats produced {} notices",
        heard(&result).len()
    );
}

/// A refused write changed nothing, so it announces nothing.
#[test]
fn a_refused_write_makes_no_news() {
    let fixture = hosted();
    let mut alex = signed_in_client(&fixture, "alex");
    let mut listener = signed_in_client(&fixture, "sam");

    let start = mark_value(&catch_up(&mut listener, None));

    // Alex has no Structure grant; this is refused.
    let refused = alex
        .invoke(Request::new("create_category", json!({ "name": "Sneaky" })))
        .unwrap();
    assert!(!refused.is_ok());

    let result = catch_up(&mut listener, Some(&start));
    assert!(
        heard(&result).is_empty(),
        "a refused write announced itself"
    );
}

#[test]
fn a_dropped_connection_announces_the_holds_it_gave_up() {
    let fixture = hosted();
    let mut jo = signed_in_client(&fixture, "jo");
    let start = mark_value(&catch_up(&mut jo, None));

    {
        let mut sam = signed_in_client(&fixture, "sam");
        lease(&mut sam, "lease_acquire", "budget", "groceries");
    } // Sam's laptop closes.

    for _ in 0..50 {
        let result = catch_up(&mut jo, Some(&start));
        if heard(&result)
            .iter()
            .any(|n| matches!(n, Notice::RecordFreed { .. }))
        {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("the badge would have stayed up after the holder disconnected");
}

#[test]
fn asking_with_no_mark_starts_from_now_rather_than_replaying_history() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");

    sam.invoke(Request::new("create_category", json!({ "name": "Before" })))
        .unwrap();

    let mut latecomer = signed_in_client(&fixture, "sam");
    let result = catch_up(&mut latecomer, None);
    assert!(
        heard(&result).is_empty(),
        "a screen that just opened was handed a backlog"
    );
}

// ----------------------------------------------------- maintenance (phase 6)

fn close_budget(client: &mut Client) -> Response {
    client
        .invoke(Request::new("maintenance_close", json!({})))
        .expect("the call should reach the host")
}

fn reopen_budget(client: &mut Client) -> Response {
    client
        .invoke(Request::new("maintenance_reopen", json!({})))
        .expect("the call should reach the host")
}

#[test]
fn a_closed_budget_refuses_writes_and_names_who_closed_it() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut jo = signed_in_client(&fixture, "jo");

    assert!(close_budget(&mut sam).is_ok());

    match jo.invoke(Request::new("lease_acquire", json!({ "kind": "budget", "record_id": "b1" }))) {
        Ok(response) => {
            // Holds sit below Write and are deliberately not gated.
            assert!(response.is_ok(), "letting go of work should still be possible");
        }
        Err(e) => panic!("unexpected transport error: {}", e.sentence()),
    }

    let refused = sam
        .invoke(Request::new("create_category", json!({ "name": "Blocked" })))
        .unwrap();
    match refused {
        Response::Err { sentence, .. } => {
            assert!(sentence.contains("Sam"), "should name the closer: {sentence}");
            assert!(sentence.contains("maintenance"), "{sentence}");
        }
        Response::Ok { .. } => panic!("a write landed while the budget was closed"),
    }
}

/// Trap #6: a closed sign is not a blackout.
#[test]
fn reads_keep_working_while_the_budget_is_closed() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    close_budget(&mut sam);

    let response = sam.invoke(Request::new("get_accounts", json!({}))).unwrap();
    assert!(
        response.is_ok(),
        "reading was blocked, which is wider than the reason for closing"
    );
}

/// The lockout this phase had to be designed around: reopening is itself a
/// write on Admin, so a naive gate would block the one command that removes
/// the sign, leaving a restart as the only way back in.
#[test]
fn a_closed_budget_can_be_reopened_without_restarting_the_host() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");

    close_budget(&mut sam);
    assert!(
        reopen_budget(&mut sam).is_ok(),
        "the budget could not be reopened while closed — the gate blocked its own release"
    );

    let response = sam
        .invoke(Request::new("create_category", json!({ "name": "After" })))
        .unwrap();
    assert!(response.is_ok(), "writes should flow again once reopened");
}

/// A sign left up by someone who has gone out must not require finding them.
#[test]
fn a_different_administrator_can_reopen() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut pat = signed_in_client(&fixture, "pat");

    close_budget(&mut sam);
    assert!(
        reopen_budget(&mut pat).is_ok(),
        "only the closer could reopen, which strands everyone if they go out"
    );

    let response = pat
        .invoke(Request::new("create_category", json!({ "name": "After" })))
        .unwrap();
    assert!(response.is_ok());
}

#[test]
fn a_member_cannot_close_the_budget() {
    let fixture = hosted();
    let mut jo = signed_in_client(&fixture, "jo");

    match close_budget(&mut jo) {
        Response::Err { sentence, .. } => {
            assert!(sentence.contains("Jo"), "{sentence}");
            assert!(sentence.contains("Admin"), "{sentence}");
        }
        Response::Ok { .. } => panic!("a member closed the budget"),
    }
}

#[test]
fn anyone_can_see_why_their_saves_are_being_refused() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut jo = signed_in_client(&fixture, "jo");

    close_budget(&mut sam);

    let response = jo
        .invoke(Request::new("maintenance_status", json!({})))
        .unwrap();
    match response {
        Response::Ok { value } => {
            assert_eq!(value["closed_by"], "Sam");
        }
        Response::Err { sentence, .. } => panic!("unexpected refusal: {sentence}"),
    }
}

#[test]
fn both_transitions_ride_the_news_so_the_banner_raises_and_lowers_itself() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut jo = signed_in_client(&fixture, "jo");

    let start = mark_value(&catch_up(&mut jo, None));
    close_budget(&mut sam);

    let result = catch_up(&mut jo, Some(&start));
    let notices = heard(&result);
    assert_eq!(notices.len(), 1);
    match &notices[0] {
        Notice::MaintenanceOn { closed_by } => assert_eq!(closed_by, "Sam"),
        other => panic!("expected MaintenanceOn, got {other:?}"),
    }

    let mid = mark_value(&result);
    reopen_budget(&mut sam);

    let result = catch_up(&mut jo, Some(&mid));
    let notices = heard(&result);
    assert_eq!(notices.len(), 1);
    assert!(matches!(notices[0], Notice::MaintenanceOff));
}

#[test]
fn closing_twice_leaves_the_first_closer_named_and_makes_no_second_announcement() {
    let fixture = hosted();
    let mut sam = signed_in_client(&fixture, "sam");
    let mut pat = signed_in_client(&fixture, "pat");
    let mut jo = signed_in_client(&fixture, "jo");

    close_budget(&mut sam);
    let mark = mark_value(&catch_up(&mut jo, None));

    assert!(close_budget(&mut pat).is_ok(), "a second close should be a harmless no-op");

    let status = pat
        .invoke(Request::new("maintenance_status", json!({})))
        .unwrap();
    match status {
        Response::Ok { value } => assert_eq!(value["closed_by"], "Sam"),
        Response::Err { sentence, .. } => panic!("unexpected refusal: {sentence}"),
    }

    let result = catch_up(&mut jo, Some(&mark));
    assert!(
        heard(&result).is_empty(),
        "a no-op close announced itself"
    );
}
