//! Structure: categories and the rules that classify transactions.
//!
//! Auto-categorising is registered under Money rather than Structure, even
//! though it is driven by Structure rules. What it *changes* is transactions,
//! and a command answers to the grant of the data it writes.

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
struct Wrapped<T> {
    request: T,
}

#[derive(Debug, Deserialize)]
struct ById {
    id: String,
}

fn h_get_categories(ctx: &BoundaryCtx, _a: Value) -> Result<Value, BoundaryError> {
    ok(ctx.db.with_connection(|c| repository::get_all_categories(c)).map_err(db_err)?)
}

fn h_get_category(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: ById = decode(args)?;
    ok(ctx.db.with_connection(|c| repository::get_category(c, &a.id)).map_err(db_err)?)
}

fn h_create_category(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let category = Category::from_request(decode::<Wrapped<_>>(args)?.request);
    ctx.db.with_connection(|c| repository::create_category(c, &category)).map_err(db_err)?;
    after_write(ctx, Written { table: Stamped::Categories, area: Area::Structure,
        record_kind: "category", id: &category.id, is_new: true, leasable: Some(Leasable::Category) })?;
    ok(category)
}

fn h_update_category(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let request: UpdateCategoryRequest = decode::<Wrapped<_>>(args.clone())?.request;
    guard_version(ctx, &args, Stamped::Categories, "category", &request.id)?;
    let id = request.id.clone();
    let category = ctx.db.with_connection(|conn| {
        let mut category = repository::get_category(conn, &id)?;
        request.apply_to(&mut category);
        repository::update_category(conn, &category)?;
        Ok(category)
    }).map_err(db_err)?;
    after_write(ctx, Written { table: Stamped::Categories, area: Area::Structure,
        record_kind: "category", id: &id, is_new: false, leasable: Some(Leasable::Category) })?;
    ok(category)
}

fn h_delete_category(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: ById = decode(args)?;
    ctx.db.with_connection(|c| repository::delete_category(c, &a.id)).map_err(db_err)?;
    after_delete(ctx, Area::Structure, "category", &a.id, Some(Leasable::Category));
    ok(serde_json::json!({ "deleted": true }))
}

// ---------------------------------------------------------------- rules

fn h_get_category_rules(ctx: &BoundaryCtx, _a: Value) -> Result<Value, BoundaryError> {
    ok(ops::ops_get_category_rules(ctx.db).map_err(db_err)?)
}

fn h_get_user_category_rules(ctx: &BoundaryCtx, _a: Value) -> Result<Value, BoundaryError> {
    ok(ops::ops_get_user_category_rules(ctx.db).map_err(db_err)?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuleArgs {
    category_id: String,
    pattern: String,
    #[serde(default)]
    field: Option<String>,
}

fn h_create_category_rule(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: RuleArgs = decode(args)?;
    let rule = ops::ops_create_category_rule(ctx.db, a.category_id, a.pattern, a.field)
        .map_err(db_err)?;
    after_write(ctx, Written { table: Stamped::CategoryRules, area: Area::Structure,
        record_kind: "rule", id: &rule.id, is_new: true, leasable: None })?;
    ok(rule)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuleId {
    rule_id: String,
}

fn h_delete_user_category_rule(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: RuleId = decode(args)?;
    let removed = ops::ops_delete_user_category_rule(ctx.db, a.rule_id.clone()).map_err(db_err)?;
    if removed {
        after_delete(ctx, Area::Structure, "rule", &a.rule_id, None);
    }
    ok(serde_json::json!({ "deleted": removed }))
}

// ------------------------------------------------- categorising the data

fn h_auto_categorize(ctx: &BoundaryCtx, _a: Value) -> Result<Value, BoundaryError> {
    let result = ops::ops_auto_categorize_transactions(ctx.db).map_err(db_err)?;
    // It rewrote transactions, so that is what other screens must re-read.
    after_delete(ctx, Area::Money, "transaction", "*", None);
    ok(result)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchArgs {
    keyword: String,
    category_id: String,
    match_uncategorized_only: bool,
    #[serde(default)]
    save_rule: Option<bool>,
}

fn h_batch_categorize(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: BatchArgs = decode(args)?;
    let result = ops::ops_batch_categorize_transactions(
        ctx.db, a.keyword, a.category_id, a.match_uncategorized_only, a.save_rule,
    ).map_err(db_err)?;
    after_delete(ctx, Area::Money, "transaction", "*", None);
    ok(result)
}

pub fn register(r: &mut Registry) {
    let w = Required::write(Area::Structure);
    let rd = Required::read(Area::Structure);

    r.register("get_categories", rd, h_get_categories);
    r.register("get_category", rd, h_get_category);
    r.register("create_category", w, h_create_category);
    r.register("update_category", w, h_update_category);
    r.register("delete_category", w, h_delete_category);

    r.register("get_category_rules", rd, h_get_category_rules);
    r.register("get_user_category_rules", rd, h_get_user_category_rules);
    r.register("create_category_rule", w, h_create_category_rule);
    r.register("delete_user_category_rule", w, h_delete_user_category_rule);

    // These write transactions, so they answer to Money.
    r.register("auto_categorize_transactions", Required::write(Area::Money), h_auto_categorize);
    r.register("batch_categorize_transactions", Required::write(Area::Money), h_batch_categorize);
}
