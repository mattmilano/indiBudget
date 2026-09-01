//! Planning: budgets, savings goals, and recurring transactions.
//!
//! Recurring bills sit here rather than under Money because they are the
//! schedule rather than the money itself. A household may well want someone
//! able to log spending without being able to change what the mortgage
//! payment is; posting an actual transaction from a schedule is a Money write.

use serde::Deserialize;
use serde_json::Value;

use super::money::guard_version;
use super::{after_delete, after_write, db_err, ok, Written};
use crate::boundary::leases::Leasable;
use crate::boundary::registry::{decode, BoundaryCtx, Registry};
use crate::boundary::{Area, BoundaryError, Required, Stamped};
use crate::commands as ops;
use crate::database::repository;
use crate::models::*;

#[derive(Debug, Deserialize)]
struct ById {
    id: String,
}

#[derive(Debug, Deserialize, Default)]
struct AsOf {
    #[serde(default)]
    as_of_date: Option<chrono::NaiveDate>,
}

#[derive(Debug, Deserialize, Default)]
struct Days {
    #[serde(default)]
    days: Option<i32>,
}

// -------------------------------------------------------------- budgets

fn h_create_budget(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let budget = Budget::from_request(decode(args)?);
    ctx.db
        .with_connection(|conn| repository::create_budget(conn, &budget))
        .map_err(db_err)?;
    after_write(ctx, Written { table: Stamped::Budgets, area: Area::Planning,
        record_kind: "budget", id: &budget.id, is_new: true, leasable: Some(Leasable::Budget) })?;
    ok(budget)
}

fn h_get_budgets(ctx: &BoundaryCtx, _a: Value) -> Result<Value, BoundaryError> {
    ok(ctx.db.with_connection(|c| repository::get_all_budgets(c)).map_err(db_err)?)
}

fn h_get_budget(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: ById = decode(args)?;
    ok(ctx.db.with_connection(|c| repository::get_budget(c, &a.id)).map_err(db_err)?)
}

fn h_update_budget(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let request: UpdateBudgetRequest = decode(args.clone())?;
    guard_version(ctx, &args, Stamped::Budgets, "budget", &request.id)?;
    let id = request.id.clone();
    let budget = ctx.db.with_connection(|conn| {
        let mut budget = repository::get_budget(conn, &id)?;
        request.apply_to(&mut budget);
        repository::update_budget(conn, &budget)?;
        Ok(budget)
    }).map_err(db_err)?;
    after_write(ctx, Written { table: Stamped::Budgets, area: Area::Planning,
        record_kind: "budget", id: &id, is_new: false, leasable: Some(Leasable::Budget) })?;
    ok(budget)
}

fn h_delete_budget(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: ById = decode(args)?;
    ctx.db.with_connection(|c| repository::delete_budget(c, &a.id)).map_err(db_err)?;
    after_delete(ctx, Area::Planning, "budget", &a.id, Some(Leasable::Budget));
    ok(serde_json::json!({ "deleted": true }))
}

fn h_budget_status(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: AsOf = decode(args).unwrap_or_default();
    ok(ops::ops_get_budget_status(ctx.db, a.as_of_date).map_err(db_err)?)
}

// ---------------------------------------------------------------- goals

fn h_create_goal(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let goal = SavingsGoal::from_request(decode(args)?);
    ctx.db.with_connection(|c| repository::create_goal(c, &goal)).map_err(db_err)?;
    after_write(ctx, Written { table: Stamped::SavingsGoals, area: Area::Planning,
        record_kind: "goal", id: &goal.id, is_new: true, leasable: Some(Leasable::Goal) })?;
    ok(goal)
}

fn h_get_goals(ctx: &BoundaryCtx, _a: Value) -> Result<Value, BoundaryError> {
    ok(ctx.db.with_connection(|c| repository::get_all_goals(c)).map_err(db_err)?)
}

fn h_get_goal(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: ById = decode(args)?;
    ok(ctx.db.with_connection(|c| repository::get_goal(c, &a.id)).map_err(db_err)?)
}

fn h_update_goal(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let request: UpdateGoalRequest = decode(args.clone())?;
    guard_version(ctx, &args, Stamped::SavingsGoals, "goal", &request.id)?;
    let id = request.id.clone();
    let goal = ctx.db.with_connection(|conn| {
        let mut goal = repository::get_goal(conn, &id)?;
        request.apply_to(&mut goal);
        repository::update_goal(conn, &goal)?;
        Ok(goal)
    }).map_err(db_err)?;
    after_write(ctx, Written { table: Stamped::SavingsGoals, area: Area::Planning,
        record_kind: "goal", id: &id, is_new: false, leasable: Some(Leasable::Goal) })?;
    ok(goal)
}

#[derive(Debug, Deserialize)]
struct GoalProgressArgs {
    id: String,
    amount: rust_decimal::Decimal,
}

fn h_update_goal_progress(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: GoalProgressArgs = decode(args)?;
    ctx.db.with_connection(|c| repository::update_goal_amount(c, &a.id, a.amount))
        .map_err(db_err)?;
    after_write(ctx, Written { table: Stamped::SavingsGoals, area: Area::Planning,
        record_kind: "goal", id: &a.id, is_new: false, leasable: Some(Leasable::Goal) })?;
    ok(serde_json::json!({ "updated": true }))
}

fn h_delete_goal(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: ById = decode(args)?;
    ctx.db.with_connection(|c| repository::delete_goal(c, &a.id)).map_err(db_err)?;
    after_delete(ctx, Area::Planning, "goal", &a.id, Some(Leasable::Goal));
    ok(serde_json::json!({ "deleted": true }))
}

// ------------------------------------------------------------ recurring

fn h_create_recurring(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let recurring = RecurringTransaction::from_request(decode(args)?);
    ctx.db.with_connection(|c| repository::create_recurring(c, &recurring)).map_err(db_err)?;
    after_write(ctx, Written { table: Stamped::RecurringTransactions, area: Area::Planning,
        record_kind: "recurring", id: &recurring.id, is_new: true, leasable: None })?;
    ok(recurring)
}

fn h_get_recurring(ctx: &BoundaryCtx, _a: Value) -> Result<Value, BoundaryError> {
    ok(ctx.db.with_connection(|c| repository::get_all_recurring(c)).map_err(db_err)?)
}

fn h_get_recurring_by_id(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: ById = decode(args)?;
    ok(ctx.db.with_connection(|c| repository::get_recurring_by_id(c, &a.id)).map_err(db_err)?)
}

fn h_update_recurring(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let request: UpdateRecurringRequest = decode(args.clone())?;
    guard_version(ctx, &args, Stamped::RecurringTransactions, "recurring payment", &request.id)?;
    let id = request.id.clone();
    let recurring = ctx.db.with_connection(|conn| {
        let mut recurring = repository::get_recurring_by_id(conn, &id)?;
        request.apply_to(&mut recurring);
        repository::update_recurring(conn, &recurring)?;
        Ok(recurring)
    }).map_err(db_err)?;
    after_write(ctx, Written { table: Stamped::RecurringTransactions, area: Area::Planning,
        record_kind: "recurring", id: &id, is_new: false, leasable: None })?;
    ok(recurring)
}

fn h_upcoming_recurring(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: Days = decode(args).unwrap_or_default();
    ok(ops::ops_get_upcoming_recurring(ctx.db, a.days).map_err(db_err)?)
}

fn h_detect_patterns(ctx: &BoundaryCtx, _a: Value) -> Result<Value, BoundaryError> {
    ok(ops::ops_detect_recurring_patterns(ctx.db).map_err(db_err)?)
}

fn h_create_from_detected(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let detected = decode(args)?;
    let created = ops::ops_create_recurring_from_detected(ctx.db, detected).map_err(db_err)?;
    after_write(ctx, Written { table: Stamped::RecurringTransactions, area: Area::Planning,
        record_kind: "recurring", id: &created.id, is_new: true, leasable: None })?;
    ok(created)
}

#[derive(Debug, Deserialize)]
struct CancelArgs {
    id: String,
    #[serde(default)]
    reason: Option<String>,
}

fn h_deactivate_recurring(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: CancelArgs = decode(args)?;
    let cancelled = ops::ops_deactivate_recurring(ctx.db, a.id.clone(), a.reason).map_err(db_err)?;
    after_delete(ctx, Area::Planning, "recurring", &a.id, None);
    ok(cancelled)
}

fn h_cancelled_subscriptions(ctx: &BoundaryCtx, _a: Value) -> Result<Value, BoundaryError> {
    ok(ops::ops_get_cancelled_subscriptions(ctx.db).map_err(db_err)?)
}

fn h_savings_summary(ctx: &BoundaryCtx, _a: Value) -> Result<Value, BoundaryError> {
    ok(ops::ops_get_savings_summary(ctx.db).map_err(db_err)?)
}

fn h_bill_reminders(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: Days = decode(args).unwrap_or_default();
    ok(ops::ops_get_bill_reminders(ctx.db, a.days).map_err(db_err)?)
}

pub fn register(r: &mut Registry) {
    let w = Required::write(Area::Planning);
    let rd = Required::read(Area::Planning);

    r.register("create_budget", w, h_create_budget);
    r.register("get_budgets", rd, h_get_budgets);
    r.register("get_budget", rd, h_get_budget);
    r.register("update_budget", w, h_update_budget);
    r.register("delete_budget", w, h_delete_budget);
    r.register("get_budget_status", rd, h_budget_status);

    r.register("create_goal", w, h_create_goal);
    r.register("get_goals", rd, h_get_goals);
    r.register("get_goal", rd, h_get_goal);
    r.register("update_goal", w, h_update_goal);
    r.register("update_goal_progress", w, h_update_goal_progress);
    r.register("delete_goal", w, h_delete_goal);

    r.register("create_recurring", w, h_create_recurring);
    r.register("get_recurring", rd, h_get_recurring);
    r.register("get_recurring_by_id", rd, h_get_recurring_by_id);
    r.register("update_recurring", w, h_update_recurring);
    r.register("get_upcoming_recurring", rd, h_upcoming_recurring);
    r.register("detect_recurring_patterns", rd, h_detect_patterns);
    r.register("create_recurring_from_detected", w, h_create_from_detected);
    r.register("deactivate_recurring", w, h_deactivate_recurring);
    r.register("get_cancelled_subscriptions", rd, h_cancelled_subscriptions);
    r.register("get_savings_summary", rd, h_savings_summary);

    // Which bills are due soon is a read of the schedule. Actually *raising* a
    // desktop notification stays on the host — see HOST_ONLY.
    r.register("get_bill_reminders", rd, h_bill_reminders);
}
