//! Tier 1 — Budget math (invariant I4) and period-boundary footgun (F12).
//!
//! I4: budget "spent" == Σ(expense transactions matching that category in the
//!     period); remaining == budget.amount − spent.
//! F12: a transaction dated on the first/last day of a period lands in that
//!     period regardless of anything.
//!
//! NOTE (verified against source): indiBudget's `calculate_budget_status` does
//! NOT apply rollover (the `rollover` flag is stored but not computed), so
//! `remaining == amount − spent` exactly. Rollover (F10) is intentionally not
//! asserted because the feature is not implemented.

mod common;

use common::*;
use indibudget_lib::models::*;
use indibudget_lib::services::reports::calculate_budget_status;

/// Helper: build a budget for a category and compute its status as-of a date.
fn status_for(
    db: &indibudget_lib::database::Database,
    category_id: &str,
    amount: &str,
    period: BudgetPeriod,
    start: &str,
    as_of: &str,
) -> BudgetStatus {
    let budget = {
        let mut b = Budget::new(
            "Test Budget".to_string(),
            category_id.to_string(),
            dec(amount),
            period,
            date(start),
        );
        b.is_active = true;
        b
    };
    db.with_connection(|conn| indibudget_lib::database::repository::create_budget(conn, &budget))
        .expect("create budget");

    let txns = all_transactions(db);
    let cats = db
        .with_connection(|conn| indibudget_lib::database::repository::get_all_categories(conn))
        .expect("categories");

    calculate_budget_status(&budget, &txns, &cats, date(as_of))
}

#[test]
fn monthly_budget_spent_and_remaining_are_hand_derived() {
    let db = db();
    let groceries = new_category(&db, "Groceries", CategoryType::Expense, "#00aa00");
    let acct = new_account(&db, "Checking", AccountType::Checking, "1000.00");

    // Three June expenses in the budgeted category: 30 + 45.50 + 12.25 = 87.75
    add_expense(&db, &acct, "30.00", "2026-06-03", "Market", Some(&groceries));
    add_expense(&db, &acct, "45.50", "2026-06-10", "Market", Some(&groceries));
    add_expense(&db, &acct, "12.25", "2026-06-20", "Market", Some(&groceries));
    // A different-category expense must NOT count.
    let dining = new_category(&db, "Dining", CategoryType::Expense, "#aa0000");
    add_expense(&db, &acct, "99.99", "2026-06-15", "Restaurant", Some(&dining));
    // An expense in a different MONTH must NOT count.
    add_expense(&db, &acct, "500.00", "2026-05-31", "May", Some(&groceries));

    let s = status_for(&db, &groceries, "200.00", BudgetPeriod::Monthly, "2026-06-01", "2026-06-15");

    // Hand-derived: spent = 87.75, remaining = 200 - 87.75 = 112.25
    assert_eq!(s.spent, dec("87.75"), "spent must sum only matching-category, in-period expenses");
    assert_eq!(s.remaining, dec("112.25"), "remaining == amount - spent");
    assert!(!s.is_over_budget);
}

#[test]
fn over_budget_is_flagged_and_remaining_goes_negative() {
    let db = db();
    let fun = new_category(&db, "Fun", CategoryType::Expense, "#0000aa");
    let acct = new_account(&db, "Checking", AccountType::Checking, "1000.00");

    add_expense(&db, &acct, "120.00", "2026-06-05", "Concert", Some(&fun));
    add_expense(&db, &acct, "60.00", "2026-06-06", "Movies", Some(&fun));

    // Budget 150; spent 180 -> remaining -30, over budget.
    let s = status_for(&db, &fun, "150.00", BudgetPeriod::Monthly, "2026-06-01", "2026-06-30");
    assert_eq!(s.spent, dec("180.00"));
    assert_eq!(s.remaining, dec("-30.00"));
    assert!(s.is_over_budget, "spent > amount must flag over budget");
}

#[test]
fn transfers_never_count_against_a_budget() {
    // I5 / F3 at the budget layer: a transfer-typed row in the budgeted
    // category window must not be counted as spending.
    let db = db();
    let groceries = new_category(&db, "Groceries", CategoryType::Expense, "#00aa00");
    let acct = new_account(&db, "Checking", AccountType::Checking, "1000.00");

    add_expense(&db, &acct, "40.00", "2026-06-10", "Market", Some(&groceries));
    // A transfer the same month — even if mis-categorized — must be ignored.
    let mut transfer = Transaction::new(
        acct.clone(),
        TransactionType::Transfer,
        dec("250.00"),
        date("2026-06-11"),
        "Move to savings".to_string(),
    );
    transfer.category_id = Some(groceries.clone());
    db.with_connection(|conn| {
        indibudget_lib::database::repository::create_transaction(conn, &transfer)
    })
    .expect("create transfer");

    let s = status_for(&db, &groceries, "100.00", BudgetPeriod::Monthly, "2026-06-01", "2026-06-30");
    assert_eq!(s.spent, dec("40.00"), "transfer must not count as budget spending");
    assert_eq!(s.remaining, dec("60.00"));
}

#[test]
fn period_boundaries_first_and_last_day_are_inside_monthly_period() {
    // F12: dates on the first and last day of June must be included.
    let db = db();
    let cat = new_category(&db, "Utilities", CategoryType::Expense, "#888888");
    let acct = new_account(&db, "Checking", AccountType::Checking, "1000.00");

    add_expense(&db, &acct, "10.00", "2026-06-01", "First day", Some(&cat)); // inclusive start
    add_expense(&db, &acct, "20.00", "2026-06-30", "Last day", Some(&cat)); // inclusive end
    add_expense(&db, &acct, "999.00", "2026-07-01", "Next month", Some(&cat)); // excluded

    let s = status_for(&db, &cat, "100.00", BudgetPeriod::Monthly, "2026-06-01", "2026-06-15");
    // Hand-derived: 10 + 20 = 30 (July 1 excluded)
    assert_eq!(s.spent, dec("30.00"), "first & last day of month must be included; next month excluded");
}

#[test]
fn yearly_period_includes_jan1_and_dec31() {
    let db = db();
    let cat = new_category(&db, "Annual", CategoryType::Expense, "#444444");
    let acct = new_account(&db, "Checking", AccountType::Checking, "1000.00");

    add_expense(&db, &acct, "100.00", "2026-01-01", "Jan 1", Some(&cat));
    add_expense(&db, &acct, "100.00", "2026-12-31", "Dec 31", Some(&cat));
    add_expense(&db, &acct, "100.00", "2025-12-31", "Prior year", Some(&cat));
    add_expense(&db, &acct, "100.00", "2027-01-01", "Next year", Some(&cat));

    let s = status_for(&db, &cat, "1000.00", BudgetPeriod::Yearly, "2026-01-01", "2026-06-15");
    // Hand-derived: only the two 2026 rows -> 200.00
    assert_eq!(s.spent, dec("200.00"), "yearly period bounds must include Jan 1 and Dec 31 of the year only");
}

#[test]
fn weekly_period_uses_monday_to_sunday_window() {
    // 2026-06-15 is a Monday. The weekly window should be Mon 06-15 .. Sun 06-21.
    let db = db();
    let cat = new_category(&db, "Coffee", CategoryType::Expense, "#553311");
    let acct = new_account(&db, "Checking", AccountType::Checking, "1000.00");

    add_expense(&db, &acct, "5.00", "2026-06-15", "Mon", Some(&cat)); // in
    add_expense(&db, &acct, "5.00", "2026-06-21", "Sun", Some(&cat)); // in
    add_expense(&db, &acct, "5.00", "2026-06-14", "Prev Sun", Some(&cat)); // out
    add_expense(&db, &acct, "5.00", "2026-06-22", "Next Mon", Some(&cat)); // out

    let s = status_for(&db, &cat, "50.00", BudgetPeriod::Weekly, "2026-06-01", "2026-06-17");
    // Hand-derived: the Monday and Sunday inside the week -> 10.00
    assert_eq!(s.spent, dec("10.00"), "weekly window should be the Mon..Sun containing as_of_date");
}
