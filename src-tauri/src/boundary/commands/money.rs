//! Money: accounts, transactions, transfers, and the bill calendar.
//!
//! The calendar lives here rather than under Reports because it hands back
//! individual transactions — descriptions, payees, amounts. A report answers to
//! the grant of the data it exposes, and this exposes rows.

use serde::Deserialize;
use serde_json::Value;

use super::{after_delete, after_write, db_err, ok, Written};
use crate::boundary::leases::{check_row_version, Leasable};
use crate::boundary::registry::{decode, BoundaryCtx, Registry};
use crate::boundary::{Area, BoundaryError, Required, Stamped};
use crate::database::repository;
use crate::models::*;

#[derive(Debug, Deserialize)]
struct ById {
    id: String,
}

#[derive(Debug, Deserialize)]
struct Dated {
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
}

// ------------------------------------------------------------- accounts

fn h_create_account(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let request: CreateAccountRequest = decode(args)?;
    // The same mapping the local screen uses, so the two cannot drift.
    let account = Account::from_request(request);

    ctx.db
        .with_connection(|conn| repository::create_account(conn, &account))
        .map_err(db_err)?;

    after_write(
        ctx,
        Written {
            table: Stamped::Accounts,
            area: Area::Money,
            record_kind: "account",
            id: &account.id,
            is_new: true,
            leasable: Some(Leasable::Account),
        },
    )?;
    ok(account)
}

fn h_get_accounts(ctx: &BoundaryCtx, _args: Value) -> Result<Value, BoundaryError> {
    ok(ctx
        .db
        .with_connection(|conn| repository::get_all_accounts(conn))
        .map_err(db_err)?)
}

fn h_get_account(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let args: ById = decode(args)?;
    ok(ctx
        .db
        .with_connection(|conn| repository::get_account(conn, &args.id))
        .map_err(db_err)?)
}

fn h_update_account(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let request: UpdateAccountRequest = decode(args.clone())?;
    guard_version(ctx, &args, Stamped::Accounts, "account", &request.id)?;

    let id = request.id.clone();
    let account = ctx
        .db
        .with_connection(|conn| {
            let mut account = repository::get_account(conn, &id)?;
            // The same merge the local screen applies.
            request.apply_to(&mut account);
            repository::update_account(conn, &account)?;
            repository::get_account(conn, &id)
        })
        .map_err(db_err)?;

    after_write(
        ctx,
        Written {
            table: Stamped::Accounts,
            area: Area::Money,
            record_kind: "account",
            id: &id,
            is_new: false,
            leasable: Some(Leasable::Account),
        },
    )?;
    ok(account)
}

fn h_delete_account(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let args: ById = decode(args)?;
    ctx.db
        .with_connection(|conn| repository::delete_account(conn, &args.id))
        .map_err(db_err)?;
    after_delete(ctx, Area::Money, "account", &args.id, Some(Leasable::Account));
    ok(serde_json::json!({ "deleted": true }))
}

// --------------------------------------------------------- transactions

fn h_create_transaction(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let request: CreateTransactionRequest = decode(args)?;
    let tx = Transaction::from_request(request);

    ctx.db
        .with_connection(|conn| repository::create_transaction(conn, &tx))
        .map_err(db_err)?;

    after_write(
        ctx,
        Written {
            table: Stamped::Transactions,
            area: Area::Money,
            record_kind: "transaction",
            id: &tx.id,
            is_new: true,
            // Transactions take no hold; see boundary::leases.
            leasable: None,
        },
    )?;
    ok(tx)
}

fn h_get_transactions(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let filter: TransactionFilter = decode(args)?;
    ok(ctx
        .db
        .with_connection(|conn| repository::get_transactions(conn, &filter))
        .map_err(db_err)?)
}

fn h_get_transaction(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let args: ById = decode(args)?;
    ok(ctx
        .db
        .with_connection(|conn| repository::get_transaction(conn, &args.id))
        .map_err(db_err)?)
}

fn h_get_transaction_count(ctx: &BoundaryCtx, _args: Value) -> Result<Value, BoundaryError> {
    ok(crate::commands::ops_transaction_count(ctx.db).map_err(db_err)?)
}

fn h_update_transaction(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let request: UpdateTransactionRequest = decode(args.clone())?;
    // Transactions have no edit hold, so this optimistic check is the only
    // thing standing between two people editing the same row.
    guard_version(ctx, &args, Stamped::Transactions, "transaction", &request.id)?;

    let id = request.id.clone();
    let tx = ctx
        .db
        .with_connection(|conn| {
            let mut tx = repository::get_transaction(conn, &id)?;
            request.apply_to(&mut tx);
            repository::update_transaction(conn, &tx)?;
            Ok(tx)
        })
        .map_err(db_err)?;

    after_write(
        ctx,
        Written {
            table: Stamped::Transactions,
            area: Area::Money,
            record_kind: "transaction",
            id: &id,
            is_new: false,
            leasable: None,
        },
    )?;
    ok(tx)
}

fn h_delete_transaction(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let args: ById = decode(args)?;
    ctx.db
        .with_connection(|conn| repository::delete_transaction_with_pair(conn, &args.id))
        .map_err(db_err)?;
    after_delete(ctx, Area::Money, "transaction", &args.id, None);
    ok(serde_json::json!({ "deleted": true }))
}

fn h_create_transfer(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let request: CreateTransferRequest = decode(args)?;
    // The shared implementation, so a transfer from a laptop takes the same
    // path as one made at the host.
    let result = crate::commands::ops_create_transfer(ctx.db, request)
        .map_err(BoundaryError::invalid)?;

    // Both sides moved, so both accounts are worth re-reading.
    after_delete(ctx, Area::Money, "transaction", &result.from_transaction_id, None);
    after_delete(ctx, Area::Money, "transaction", &result.to_transaction_id, None);
    ok(result)
}

// ------------------------------------------------------------- calendar

fn h_get_calendar_events(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let args: Dated = decode(args)?;
    ok(
        crate::commands::ops_calendar_events(ctx.db, args.start_date, args.end_date)
            .map_err(db_err)?,
    )
}

// --------------------------------------------------------------- shared

/// Apply the optimistic check when the caller supplied a version.
///
/// A caller that sends `expected_row_version` is asking to be told if the row
/// moved under them. One that omits it is taking the older last-write-wins
/// behaviour, which is what the local screens still do until they carry the
/// column.
pub(super) fn guard_version(
    ctx: &BoundaryCtx,
    args: &Value,
    table: Stamped,
    label: &str,
    id: &str,
) -> Result<(), BoundaryError> {
    let Some(expected) = args.get("expected_row_version").and_then(|v| v.as_i64()) else {
        return Ok(());
    };
    ctx.db
        .with_connection(|conn| Ok(check_row_version(conn, table, label, id, expected)))
        .map_err(db_err)?
}

// `delete_transaction` uses the pair-aware repository call so that deleting one
// side of a transfer takes the other with it, exactly as the local screen does.

pub fn register(registry: &mut Registry) {
    registry.register("create_account", Required::write(Area::Money), h_create_account);
    registry.register("get_accounts", Required::read(Area::Money), h_get_accounts);
    registry.register("get_account", Required::read(Area::Money), h_get_account);
    registry.register("update_account", Required::write(Area::Money), h_update_account);
    registry.register("delete_account", Required::write(Area::Money), h_delete_account);

    registry.register(
        "create_transaction",
        Required::write(Area::Money),
        h_create_transaction,
    );
    registry.register("get_transactions", Required::read(Area::Money), h_get_transactions);
    registry.register("get_transaction", Required::read(Area::Money), h_get_transaction);
    registry.register(
        "get_transaction_count",
        Required::read(Area::Money),
        h_get_transaction_count,
    );
    registry.register(
        "update_transaction",
        Required::write(Area::Money),
        h_update_transaction,
    );
    registry.register(
        "delete_transaction",
        Required::write(Area::Money),
        h_delete_transaction,
    );
    registry.register("create_transfer", Required::write(Area::Money), h_create_transfer);

    // Row-level data, so it answers to Money rather than Reports.
    registry.register(
        "get_calendar_events",
        Required::read(Area::Money),
        h_get_calendar_events,
    );
}
