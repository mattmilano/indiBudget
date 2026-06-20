//! Tier 3 — Reports tie-out (invariant I9) and transfer-exclusion (I5 / F3).
//!
//! I9: spending-by-category percentages sum to 100% (±rounding); monthly
//!     net == income − expense; cash-flow running balance ends at
//!     starting_balance + net.
//! I5/F3: transfers never appear as income/expense in reports.

mod common;

use common::*;
use indibudget_lib::models::*;
use indibudget_lib::services::reports::{
    calculate_cash_flow, calculate_monthly_trends, calculate_spending_by_category,
};

fn categories(db: &indibudget_lib::database::Database) -> Vec<Category> {
    db.with_connection(|conn| indibudget_lib::database::repository::get_all_categories(conn))
        .expect("categories")
}

#[test]
fn spending_by_category_percentages_sum_to_100() {
    let db = db();
    let a = new_category(&db, "A", CategoryType::Expense, "#111111");
    let b = new_category(&db, "B", CategoryType::Expense, "#222222");
    let acct = new_account(&db, "Checking", AccountType::Checking, "0.00");

    add_expense(&db, &acct, "75.00", "2026-06-01", "a1", Some(&a));
    add_expense(&db, &acct, "25.00", "2026-06-02", "b1", Some(&b));

    let rows = calculate_spending_by_category(&all_transactions(&db), &categories(&db));
    let total_pct: f64 = rows.iter().map(|r| r.percentage).sum();
    assert!((total_pct - 100.0).abs() < 0.001, "percentages must sum to 100, got {total_pct}");

    // Hand-derived totals: A = 75, B = 25.
    let a_row = rows.iter().find(|r| r.category_id == a).unwrap();
    let b_row = rows.iter().find(|r| r.category_id == b).unwrap();
    assert_eq!(a_row.total, dec("75.00"));
    assert_eq!(b_row.total, dec("25.00"));
    assert!((a_row.percentage - 75.0).abs() < 0.001);
    assert!((b_row.percentage - 25.0).abs() < 0.001);
}

#[test]
fn spending_by_category_excludes_income_and_transfers() {
    // I5 / F3: only expenses count toward spending-by-category.
    let db = db();
    let cat = new_category(&db, "Groceries", CategoryType::Expense, "#00aa00");
    let acct = new_account(&db, "Checking", AccountType::Checking, "0.00");

    add_expense(&db, &acct, "40.00", "2026-06-01", "expense", Some(&cat));
    add_income(&db, &acct, "1000.00", "2026-06-01", "salary", Some(&cat));
    // transfer mis-tagged into an expense category must still be ignored
    let mut t = Transaction::new(
        acct.clone(),
        TransactionType::Transfer,
        dec("500.00"),
        date("2026-06-01"),
        "xfer".to_string(),
    );
    t.category_id = Some(cat.clone());
    db.with_connection(|conn| indibudget_lib::database::repository::create_transaction(conn, &t))
        .expect("create transfer");

    let rows = calculate_spending_by_category(&all_transactions(&db), &categories(&db));
    let cat_row = rows.iter().find(|r| r.category_id == cat).unwrap();
    assert_eq!(cat_row.total, dec("40.00"), "only the expense should count");
    assert_eq!(cat_row.transaction_count, 1);
}

#[test]
fn monthly_trend_net_equals_income_minus_expense_and_ignores_transfers() {
    let db = db();
    let checking = new_account(&db, "Checking", AccountType::Checking, "0.00");
    let savings = new_account(&db, "Savings", AccountType::Savings, "0.00");

    add_income(&db, &checking, "3000.00", "2026-06-05", "salary", None);
    add_expense(&db, &checking, "1200.00", "2026-06-10", "rent", None);
    add_expense(&db, &checking, "300.00", "2026-06-15", "food", None);
    add_transfer(&db, &checking, &savings, "999.00", "2026-06-20", "to savings"); // must be ignored

    let trends = calculate_monthly_trends(&all_transactions(&db), 12);
    let june = trends
        .iter()
        .find(|t| t.year == 2026 && t.month == "Jun")
        .expect("June trend present");

    // Hand-derived: income 3000, expense 1500, net 1500.
    assert_eq!(june.income, dec("3000.00"));
    assert_eq!(june.expenses, dec("1500.00"));
    assert_eq!(june.net, dec("1500.00"));
    assert_eq!(june.net, june.income - june.expenses, "net invariant");
}

#[test]
fn cash_flow_running_balance_reconciles_to_starting_plus_net() {
    // I9: ending running balance == starting_balance + (income − expense) over
    // the period. Transfers excluded from income/expense totals.
    let db = db();
    let checking = new_account(&db, "Checking", AccountType::Checking, "0.00");
    let savings = new_account(&db, "Savings", AccountType::Savings, "0.00");

    add_income(&db, &checking, "1000.00", "2026-06-01", "salary", None);
    add_expense(&db, &checking, "200.00", "2026-06-02", "groceries", None);
    add_expense(&db, &checking, "50.00", "2026-06-03", "gas", None);
    add_transfer(&db, &checking, &savings, "400.00", "2026-06-02", "to savings"); // ignored

    let start = date("2026-06-01");
    let end = date("2026-06-30");
    let starting_balance = dec("500.00");
    let report = calculate_cash_flow(
        &all_transactions(&db),
        &categories(&db),
        start,
        end,
        starting_balance,
    );

    // Hand-derived: income 1000, expense 250, net 750.
    assert_eq!(report.total_income, dec("1000.00"));
    assert_eq!(report.total_expenses, dec("250.00"));
    assert_eq!(report.net_cash_flow, dec("750.00"));

    // Ending running balance == 500 + 750 == 1250.
    let last = report.daily_balances.last().expect("at least one daily balance");
    assert_eq!(last.balance, dec("1250.00"), "cash-flow balance must reconcile");
    assert_eq!(
        last.balance,
        starting_balance + report.net_cash_flow,
        "ending balance == starting + net"
    );
}
