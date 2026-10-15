//! Editing a scheduled transaction in place, rather than deleting it and
//! making it again — which loses its history and is easy to get wrong.
//!
//! Driven through the same boundary the screens use.

use indibudget_lib::boundary::commands::build_registry;
use indibudget_lib::boundary::registry::{dispatch, BoundaryCtx};
use indibudget_lib::boundary::{Actor, Request, Response, SharedState};
use indibudget_lib::database::Database;
use serde_json::{json, Value};

struct Budget {
    db: Database,
    shared: SharedState,
}

impl Budget {
    fn new() -> Self {
        Budget { db: Database::in_memory().unwrap(), shared: SharedState::new() }
    }

    fn ask(&self, command: &str, args: Value) -> Result<Value, String> {
        let registry = build_registry();
        let actor = Actor::local_owner();
        let ctx = BoundaryCtx::new(&self.db, &actor, &self.shared);
        match dispatch(&registry, &ctx, Request::new(command, args)) {
            Response::Ok { value } => Ok(value),
            Response::Err { sentence, .. } => Err(sentence),
        }
    }

    fn call(&self, command: &str, args: Value) -> Value {
        self.ask(command, args).unwrap_or_else(|e| panic!("{command} refused: {e}"))
    }

    fn account(&self, name: &str) -> String {
        self.call(
            "create_account",
            json!({ "request": { "name": name, "account_type": "checking", "balance": "0" } }),
        )["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn edit(&self, fields: Value, version: i64) -> Result<Value, String> {
        self.ask("update_recurring", json!({ "request": fields, "expected_row_version": version }))
    }

    fn scheduled_dates(&self, id: &str, from: &str, to: &str) -> Vec<String> {
        self.call("get_calendar_events", json!({ "startDate": from, "endDate": to }))
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["recurring_id"].as_str() == Some(id))
            .map(|e| e["date"].as_str().unwrap().to_string())
            .collect()
    }
}

fn version(v: &Value) -> i64 {
    v["row_version"].as_i64().unwrap()
}

fn setup() -> (Budget, String, String, Value) {
    let budget = Budget::new();
    let checking = budget.account("Checking");
    let savings = budget.account("Savings");
    let bill = budget.call(
        "create_recurring",
        json!({ "request": {
            "account_id": checking, "transaction_type": "expense", "amount": "85.00",
            "description": "Electric", "frequency": "monthly", "start_date": "2026-11-15",
            "day_of_month": 15
        } }),
    );
    (budget, checking, savings, bill)
}

#[test]
fn every_part_of_a_scheduled_transaction_can_be_changed_in_place() {
    let (budget, _checking, savings, bill) = setup();
    let id = bill["id"].as_str().unwrap().to_string();

    let edited = budget
        .edit(
            json!({
                "id": id, "description": "Electric (City Power)", "amount": "92.40",
                "account_id": savings, "transaction_type": "income",
                "payee": "City Power", "frequency": "quarterly"
            }),
            version(&bill),
        )
        .unwrap();
    assert_eq!(edited["id"], bill["id"], "the same schedule, not a new one");
    assert_eq!(edited["description"], "Electric (City Power)");
    assert_eq!(edited["amount"], "92.40");
    assert_eq!(edited["account_id"].as_str(), Some(savings.as_str()));
    assert_eq!(edited["transaction_type"], "income");
    assert_eq!(edited["payee"], "City Power");
    assert_eq!(edited["frequency"], "quarterly");
    assert_eq!(edited["next_occurrence"], bill["next_occurrence"], "untouched fields stay");
}

#[test]
fn an_end_date_and_a_payee_can_be_set_and_cleared_again() {
    let (budget, _, _, bill) = setup();
    let id = bill["id"].as_str().unwrap();

    let with_end = budget
        .edit(json!({ "id": id, "end_date": "2027-06-30", "payee": "City Power" }), version(&bill))
        .unwrap();
    assert_eq!(with_end["end_date"], "2027-06-30");

    let cleared = budget
        .edit(json!({ "id": id, "end_date": null, "payee": null }), version(&with_end))
        .unwrap();
    assert!(cleared["end_date"].is_null(), "an end date, once set, must be removable");
    assert!(cleared["payee"].is_null());
}

#[test]
fn moving_the_next_due_date_moves_the_ones_after_it() {
    let (budget, _, _, bill) = setup();
    let id = bill["id"].as_str().unwrap();
    assert_eq!(
        budget.scheduled_dates(id, "2026-11-01", "2027-01-31"),
        vec!["2026-11-15", "2026-12-15", "2027-01-15"]
    );

    // What the edit form sends when the date is changed on a monthly bill.
    budget
        .edit(json!({ "id": id, "next_occurrence": "2026-11-20", "day_of_month": 20 }), version(&bill))
        .unwrap();
    assert_eq!(
        budget.scheduled_dates(id, "2026-11-01", "2027-01-31"),
        vec!["2026-11-20", "2026-12-20", "2027-01-20"]
    );
}

#[test]
fn nonsense_is_refused_in_words_and_nothing_is_saved() {
    let (budget, _, _, bill) = setup();
    let id = bill["id"].as_str().unwrap();

    let err = budget.edit(json!({ "id": id, "end_date": "2026-11-01" }), version(&bill)).unwrap_err();
    assert!(err.contains("after the end date"), "{err}");
    let err = budget.edit(json!({ "id": id, "amount": "0" }), version(&bill)).unwrap_err();
    assert!(err.contains("more than zero"), "{err}");
    let err = budget.edit(json!({ "id": id, "description": "  " }), version(&bill)).unwrap_err();
    assert!(err.contains("description"), "{err}");

    let unchanged = budget.call("get_recurring_by_id", json!({ "id": id }));
    assert_eq!(unchanged["amount"], "85.00");
    assert!(unchanged["end_date"].is_null());
}

#[test]
fn an_edit_built_on_an_old_copy_is_refused_rather_than_overwriting() {
    let (budget, _, _, bill) = setup();
    let id = bill["id"].as_str().unwrap();
    budget.edit(json!({ "id": id, "amount": "90.00" }), version(&bill)).unwrap();

    // Someone else's form, still holding the version from before that save.
    let err = budget.edit(json!({ "id": id, "amount": "70.00" }), version(&bill)).unwrap_err();
    assert!(!err.is_empty());
    let current = budget.call("get_recurring_by_id", json!({ "id": id }));
    assert_eq!(current["amount"], "90.00");
}
