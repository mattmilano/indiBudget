//! Multi-user phase 7: the registry held to its contract as a whole.
//!
//! With this many registered commands, a test per command cannot keep up and
//! would rot. Instead these hold the entire table to a handful of properties at
//! once — which means every command registered from here on is covered by them
//! for free, without anyone remembering to add a test.

use serde_json::Value;
use std::collections::BTreeSet;

use indibudget_lib::boundary::commands::{build_registry, HOST_ONLY};
use indibudget_lib::boundary::news::CatchUp;
use indibudget_lib::boundary::registry::{dispatch, BoundaryCtx};
use indibudget_lib::boundary::{
    Access, Actor, Area, BoundaryError, Grants, Request, Response, SharedState,
};
use indibudget_lib::database::Database;

/// The only commands allowed to register with `Required::signed_in()`.
///
/// That constructor defers authorization into the handler, which is right for
/// commands whose area depends on their arguments — and dangerous anywhere
/// else. Pinning the list here means a future command cannot quietly opt out of
/// the registry's own gate.
const DEFERRED_AUTHORIZATION: &[&str] = &[
    // Area follows from which kind of record is being held.
    "lease_acquire",
    "lease_renew",
    "lease_release",
    "lease_holders",
    // Filtered per-area inside the handler; everyone may ask.
    "news_catch_up",
    // Anyone may see why their saves are being refused.
    "maintenance_status",
];

/// Commands that legitimately do their work with no arguments at all.
///
/// Everything else must refuse null arguments rather than act on them.
const NO_ARGUMENT_WRITES: &[&str] = &[
    // Re-runs the categoriser over everything; genuinely takes no arguments.
    "auto_categorize_transactions",
    // The closed sign is a toggle, not a form.
    "maintenance_close",
    "maintenance_reopen",
];

fn nobody() -> Actor {
    Actor::new("u-nobody".into(), "Nobody".into(), false, Grants::none())
}

#[test]
fn command_names_are_unique() {
    let registry = build_registry();
    let mut seen = BTreeSet::new();
    for name in registry.names() {
        assert!(seen.insert(*name), "duplicate command name: {name}");
    }
    assert!(registry.len() > 40, "the registry looks unexpectedly small");
}

/// The safe default: anything not registered cannot be reached from another
/// machine. These are the commands that must never drift into the table.
#[test]
fn host_only_commands_are_never_registered() {
    let registry = build_registry();
    for (name, why) in HOST_ONLY {
        assert!(
            !registry.contains(name),
            "{name} was registered, but it must stay on the host: {why}"
        );
    }
}

/// Every command that declares a real grant must refuse someone holding none,
/// and the refusal must name them rather than saying only "denied".
#[test]
fn a_person_with_no_grants_is_refused_at_every_gated_door() {
    let registry = build_registry();
    let db = Database::in_memory().unwrap();
    let actor = nobody();
    let shared = SharedState::new();
    let ctx = BoundaryCtx::new(&db, &actor, &shared);

    for registration in registry.all() {
        if registration.required.access < Access::Read {
            continue; // authorization is deferred; covered by its own test
        }
        let response = dispatch(&registry, &ctx, Request::new(registration.name, Value::Null));
        match response {
            Response::Err { error, sentence } => {
                assert!(
                    matches!(error, BoundaryError::Denied { .. }),
                    "{} refused with {error:?}, expected a grant refusal",
                    registration.name
                );
                assert!(
                    sentence.contains("Nobody"),
                    "{} did not name the person: {sentence}",
                    registration.name
                );
            }
            Response::Ok { .. } => {
                panic!("{} admitted someone with no grants", registration.name)
            }
        }
    }
}

/// `Required::signed_in()` defers the real check into the handler. That is
/// correct for a few commands and a hole anywhere else, so the set is pinned.
#[test]
fn only_the_expected_commands_defer_their_authorization() {
    let registry = build_registry();
    let deferred: BTreeSet<&str> = registry
        .all()
        .filter(|r| r.required.access < Access::Read)
        .map(|r| r.name)
        .collect();
    let expected: BTreeSet<&str> = DEFERRED_AUTHORIZATION.iter().copied().collect();

    assert_eq!(
        deferred, expected,
        "a command changed how it is authorized; if that is deliberate, \
         update DEFERRED_AUTHORIZATION and say why in the commit"
    );
}

/// A read grant must not reach a write anywhere in the table.
#[test]
fn a_reader_can_reach_no_write() {
    let registry = build_registry();
    let db = Database::in_memory().unwrap();
    let mut grants = Grants::none();
    for area in Area::ALL {
        grants = grants.with(area, Access::Read);
    }
    let actor = Actor::new("u-reader".into(), "Reader".into(), false, grants);
    let shared = SharedState::new();
    let ctx = BoundaryCtx::new(&db, &actor, &shared);

    for registration in registry.all() {
        if registration.required.access != Access::Write {
            continue;
        }
        let response = dispatch(&registry, &ctx, Request::new(registration.name, Value::Null));
        match response {
            Response::Err { error, .. } => assert!(
                matches!(error, BoundaryError::Denied { .. }),
                "{} refused a reader with {error:?}, expected a grant refusal",
                registration.name
            ),
            Response::Ok { .. } => panic!(
                "{} let a read-only person write",
                registration.name
            ),
        }
    }
}

/// Null arguments must never panic. The test completing at all is most of the
/// assertion; the rest is that nothing writes by accident.
#[test]
fn null_arguments_never_panic_and_only_the_expected_commands_act_on_them() {
    let registry = build_registry();
    let db = Database::in_memory().unwrap();
    let actor = Actor::local_owner();
    let shared = SharedState::new();
    let ctx = BoundaryCtx::new(&db, &actor, &shared);

    let mut succeeded = Vec::new();
    for registration in registry.all() {
        let response = dispatch(&registry, &ctx, Request::new(registration.name, Value::Null));
        if response.is_ok() {
            succeeded.push(registration.name);
        }
    }

    // Anything that took an argument should have refused; anything that
    // succeeded must be a genuine no-argument command.
    for name in &succeeded {
        let registration = registry.get(name).unwrap();
        let is_write = registration.required.access == Access::Write;
        if is_write {
            assert!(
                NO_ARGUMENT_WRITES.contains(name),
                "{name} wrote on null arguments; if it genuinely takes none, \
                 add it to NO_ARGUMENT_WRITES"
            );
        }
    }
}

/// A refused call changed nothing, so it must announce nothing — checked
/// across the whole table at once rather than command by command.
#[test]
fn refused_calls_make_no_news_anywhere_in_the_table() {
    let registry = build_registry();
    let db = Database::in_memory().unwrap();
    let actor = nobody();
    let shared = SharedState::new();
    let ctx = BoundaryCtx::new(&db, &actor, &shared);

    let mark = shared.news.current_mark();
    for registration in registry.all() {
        let _ = dispatch(&registry, &ctx, Request::new(registration.name, Value::Null));
    }

    match shared.news.catch_up(&mark, &Actor::local_owner()) {
        CatchUp::Notices { notices, .. } => assert!(
            notices.is_empty(),
            "refused calls produced {} notices: {notices:?}",
            notices.len()
        ),
        CatchUp::StartOver { .. } => panic!("unexpected start_over"),
    }
}

/// A report answers to the grant of the data it exposes.
///
/// The Reports area is for aggregates — totals and per-category sums. Anything
/// handing back individual transactions belongs under Money, which is why the
/// bill calendar is registered there despite looking like a report.
#[test]
fn reports_expose_aggregates_and_row_level_views_answer_to_their_own_area() {
    let registry = build_registry();

    for name in ["get_spending_by_category", "get_monthly_trends", "get_cash_flow_report"] {
        let registration = registry.get(name).expect(name);
        assert_eq!(
            registration.required.area,
            Area::Reports,
            "{name} should answer to Reports"
        );
    }

    let calendar = registry.get("get_calendar_events").expect("calendar");
    assert_eq!(
        calendar.required.area,
        Area::Money,
        "the calendar hands back individual transactions, so it answers to Money"
    );
}

/// Writes that change transactions answer to Money even when they are driven
/// by Structure rules — a command answers to the grant of the data it writes.
#[test]
fn categorising_answers_to_the_data_it_rewrites() {
    let registry = build_registry();
    for name in ["auto_categorize_transactions", "batch_categorize_transactions"] {
        let registration = registry.get(name).expect(name);
        assert_eq!(registration.required.area, Area::Money, "{name}");
        assert_eq!(registration.required.access, Access::Write, "{name}");
    }
}

/// Only the commands that operate the closed sign may run while it is up.
#[test]
fn only_the_maintenance_commands_are_exempt_from_maintenance() {
    let registry = build_registry();
    let exempt: BTreeSet<&str> = registry
        .all()
        .filter(|r| r.allowed_during_maintenance)
        .map(|r| r.name)
        .collect();
    let expected: BTreeSet<&str> = ["maintenance_close", "maintenance_reopen"]
        .into_iter()
        .collect();
    assert_eq!(exempt, expected);
}
