//! Reports.
//!
//! Every command here returns **aggregates only** — totals, per-category sums,
//! per-month figures, daily balances. No descriptions, no payees, no rows.
//!
//! That is what lets them answer to a Reports grant rather than to Money. The
//! rule is that a report answers to the grant of the data it exposes, so a
//! report that ever starts handing back individual transactions belongs under
//! Money instead. `get_calendar_events` is exactly that case and lives there.

use serde::Deserialize;
use serde_json::Value;

use super::{db_err, ok};
use crate::boundary::registry::{decode, BoundaryCtx, Registry};
use crate::boundary::{Area, BoundaryError, Required};
use crate::commands as ops;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Range {
    #[serde(default)]
    start_date: Option<chrono::NaiveDate>,
    #[serde(default)]
    end_date: Option<chrono::NaiveDate>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequiredRange {
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
}

#[derive(Debug, Deserialize, Default)]
struct Months {
    #[serde(default)]
    months: Option<usize>,
}

fn h_spending_by_category(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: Range = decode(args).unwrap_or_default();
    ok(ops::ops_get_spending_by_category(ctx.db, a.start_date, a.end_date).map_err(db_err)?)
}

fn h_monthly_trends(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: Months = decode(args).unwrap_or_default();
    ok(ops::ops_get_monthly_trends(ctx.db, a.months).map_err(db_err)?)
}

fn h_cash_flow(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: RequiredRange = decode(args)?;
    ok(ops::ops_get_cash_flow_report(ctx.db, a.start_date, a.end_date).map_err(db_err)?)
}

pub fn register(r: &mut Registry) {
    let rd = Required::read(Area::Reports);
    r.register("get_spending_by_category", rd, h_spending_by_category);
    r.register("get_monthly_trends", rd, h_monthly_trends);
    r.register("get_cash_flow_report", rd, h_cash_flow);
}
