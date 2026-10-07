//! Edit holds as the screens take them.
//!
//! The edit dialogs call `lease_acquire`, `lease_renew` and `lease_release`
//! through the same local boundary every other command uses, with the argument
//! shape `api.ts` sends. These tests drive exactly that shape, so a renamed
//! field on either side shows up here rather than as a form that never unlocks.

use serde_json::{json, Value};

use indibudget_lib::boundary::commands::build_registry;
use indibudget_lib::boundary::registry::{dispatch, BoundaryCtx};
use indibudget_lib::boundary::{Actor, BoundaryError, Grants, Request, Response, SharedState};
use indibudget_lib::database::Database;

fn call(
    db: &Database,
    shared: &SharedState,
    actor: &Actor,
    command: &str,
    args: Value,
) -> Result<Value, BoundaryError> {
    let registry = build_registry();
    let ctx = BoundaryCtx::new(db, actor, shared);
    match dispatch(&registry, &ctx, Request::new(command, args)) {
        Response::Ok { value } => Ok(value),
        Response::Err { error, .. } => Err(error),
    }
}

/// Single-computer use must not change: the person at this machine always gets
/// the hold, for every kind of record the screens hold.
#[test]
fn the_person_at_this_computer_always_gets_the_hold() {
    let db = Database::in_memory().unwrap();
    let shared = SharedState::new();
    let me = Actor::local_owner();

    for kind in ["account", "budget", "category", "goal"] {
        let args = json!({ "kind": kind, "recordId": "r1" });
        assert_eq!(
            call(&db, &shared, &me, "lease_acquire", args.clone()).unwrap(),
            json!({ "held": true }),
            "{kind}"
        );
        // Opening the same record again — a second window, or the dialog
        // reopened before the release landed — is a renewal, not a refusal.
        call(&db, &shared, &me, "lease_acquire", args.clone()).unwrap();
        call(&db, &shared, &me, "lease_renew", args.clone()).unwrap();
        assert_eq!(
            call(&db, &shared, &me, "lease_release", args).unwrap(),
            json!({ "held": false })
        );
    }
}

#[test]
fn someone_else_is_told_who_is_editing() {
    let db = Database::in_memory().unwrap();
    let shared = SharedState::new();
    let host = Actor::local_owner();
    let alex = Actor::new("u-alex".into(), "Alex".into(), false, Grants::all());
    let args = json!({ "kind": "budget", "recordId": "b1" });

    call(&db, &shared, &host, "lease_acquire", args.clone()).unwrap();

    let err = call(&db, &shared, &alex, "lease_acquire", args.clone()).unwrap_err();
    assert!(matches!(err, BoundaryError::Busy { .. }), "{err:?}");
    assert_eq!(err.sentence(), "This computer is editing this budget right now.");

    // The wire form carries the holder by name, which is what the form shows.
    let wire = serde_json::to_value(Response::err(err)).unwrap();
    assert_eq!(wire["error"]["kind"], json!("busy"));
    assert_eq!(wire["error"]["holder"], json!("This computer"));

    // Once let go, the next person gets it.
    call(&db, &shared, &host, "lease_release", args.clone()).unwrap();
    call(&db, &shared, &alex, "lease_acquire", args).unwrap();
}
