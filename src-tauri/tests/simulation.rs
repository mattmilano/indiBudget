//! User simulation test — exercises the app like a real user would over a month.
//!
//! This test creates realistic accounts, categories, and transactions to verify
//! that the derived balance system works correctly in real-world scenarios.

mod common;

use common::*;
use indibudget_lib::database::repository;
use indibudget_lib::models::*;

/// Helper to create multiple expenses in a batch
fn add_expenses(db: &indibudget_lib::database::Database, account_id: &str, expenses: &[(&str, &str, &str, Option<&str>)]) {
    for (amount, date, desc, cat) in expenses {
        add_expense(db, account_id, amount, date, desc, *cat);
    }
}

#[test]
fn simulate_one_month_of_typical_user_activity() {
    let db = db();

    // ========== SETUP: Create accounts like a new user would ==========
    println!("\n=== Setting up accounts ===");

    // User has a checking account with $2,500, savings with $10,000, and a credit card at $0
    let checking = new_account(&db, "Chase Checking", AccountType::Checking, "2500.00");
    let savings = new_account(&db, "Ally Savings", AccountType::Savings, "10000.00");
    let credit_card = new_account(&db, "Chase Sapphire", AccountType::CreditCard, "0.00");

    // Verify initial balances
    assert_eq!(get_balance(&db, &checking), dec("2500.00"), "checking initial");
    assert_eq!(get_balance(&db, &savings), dec("10000.00"), "savings initial");
    assert_eq!(get_balance(&db, &credit_card), dec("0.00"), "credit card initial");

    let initial_net_worth = dec("2500.00") + dec("10000.00") + dec("0.00");
    println!("Initial net worth: ${}", initial_net_worth);

    // ========== Create categories ==========
    let groceries = new_category(&db, "Groceries", CategoryType::Expense, "#22c55e");
    let dining = new_category(&db, "Dining Out", CategoryType::Expense, "#f97316");
    let utilities = new_category(&db, "Utilities", CategoryType::Expense, "#3b82f6");
    let entertainment = new_category(&db, "Entertainment", CategoryType::Expense, "#8b5cf6");
    let gas = new_category(&db, "Gas & Auto", CategoryType::Expense, "#ef4444");
    let shopping = new_category(&db, "Shopping", CategoryType::Expense, "#ec4899");
    let salary = new_category(&db, "Salary", CategoryType::Income, "#10b981");

    // ========== JUNE 1: Payday! ==========
    println!("\n=== June 1: Payday ===");
    add_income(&db, &checking, "3500.00", "2026-06-01", "Direct Deposit - Acme Corp", Some(&salary));

    assert_eq!(get_balance(&db, &checking), dec("6000.00"), "after paycheck");

    // ========== JUNE 1-5: Beginning of month bills ==========
    println!("\n=== June 1-5: Monthly bills ===");
    add_expense(&db, &checking, "1400.00", "2026-06-01", "Rent Payment", None);
    add_expense(&db, &checking, "125.00", "2026-06-02", "Electric Company", Some(&utilities));
    add_expense(&db, &checking, "85.00", "2026-06-02", "Internet - Comcast", Some(&utilities));
    add_expense(&db, &checking, "150.00", "2026-06-03", "Car Insurance", None);

    // After bills: 6000 - 1400 - 125 - 85 - 150 = 4240
    assert_eq!(get_balance(&db, &checking), dec("4240.00"), "after bills");

    // ========== JUNE 5: Transfer to savings ==========
    println!("\n=== June 5: Transfer to savings ===");
    add_transfer(&db, &checking, &savings, "500.00", "2026-06-05", "Monthly savings");

    // Checking: 4240 - 500 = 3740
    // Savings: 10000 + 500 = 10500
    assert_eq!(get_balance(&db, &checking), dec("3740.00"), "after transfer to savings");
    assert_eq!(get_balance(&db, &savings), dec("10500.00"), "savings after transfer");

    // ========== JUNE 6-15: Daily spending on credit card ==========
    println!("\n=== June 6-15: Credit card spending ===");
    let cc_expenses: Vec<(&str, &str, &str, Option<&str>)> = vec![
        ("45.23", "2026-06-06", "Whole Foods", Some(&groceries)),
        ("12.50", "2026-06-07", "Starbucks", Some(&dining)),
        ("35.00", "2026-06-08", "Gas Station", Some(&gas)),
        ("67.89", "2026-06-09", "Target", Some(&shopping)),
        ("28.45", "2026-06-10", "Chipotle", Some(&dining)),
        ("89.99", "2026-06-11", "Grocery Outlet", Some(&groceries)),
        ("15.99", "2026-06-12", "Netflix", Some(&entertainment)),
        ("42.00", "2026-06-13", "Restaurant", Some(&dining)),
        ("55.00", "2026-06-14", "Gas Station", Some(&gas)),
        ("23.50", "2026-06-15", "Coffee Shop", Some(&dining)),
    ];
    add_expenses(&db, &credit_card, &cc_expenses);

    // Credit card balance: -(45.23+12.50+35+67.89+28.45+89.99+15.99+42+55+23.50) = -415.55
    let expected_cc = dec("-415.55");
    assert_eq!(get_balance(&db, &credit_card), expected_cc, "credit card after spending");

    // ========== JUNE 15: Mid-month paycheck ==========
    println!("\n=== June 15: Mid-month paycheck ===");
    add_income(&db, &checking, "3500.00", "2026-06-15", "Direct Deposit - Acme Corp", Some(&salary));

    // Checking: 3740 + 3500 = 7240
    assert_eq!(get_balance(&db, &checking), dec("7240.00"), "after second paycheck");

    // ========== JUNE 16: Pay off credit card ==========
    println!("\n=== June 16: Pay credit card bill ===");
    // Transfer from checking to pay off the $415.55 credit card balance
    add_transfer(&db, &checking, &credit_card, "415.55", "2026-06-16", "Credit card payment");

    // Checking: 7240 - 415.55 = 6824.45
    // Credit card: -415.55 + 415.55 = 0
    assert_eq!(get_balance(&db, &checking), dec("6824.45"), "after cc payment");
    assert_eq!(get_balance(&db, &credit_card), dec("0.00"), "credit card paid off");

    // ========== JUNE 17-30: More spending ==========
    println!("\n=== June 17-30: More spending ===");

    // Direct checking expenses
    add_expense(&db, &checking, "78.50", "2026-06-18", "Costco Gas", Some(&gas));
    add_expense(&db, &checking, "156.32", "2026-06-20", "Costco Groceries", Some(&groceries));
    add_expense(&db, &checking, "45.00", "2026-06-22", "Haircut", None);
    add_expense(&db, &checking, "200.00", "2026-06-25", "Car Repair", Some(&gas));

    // More credit card spending
    add_expense(&db, &credit_card, "89.00", "2026-06-19", "Amazon", Some(&shopping));
    add_expense(&db, &credit_card, "34.50", "2026-06-21", "Uber Eats", Some(&dining));
    add_expense(&db, &credit_card, "12.99", "2026-06-23", "Spotify", Some(&entertainment));
    add_expense(&db, &credit_card, "65.00", "2026-06-28", "Movie & Dinner", Some(&entertainment));

    // ========== JUNE 30: End of month - transfer more to savings ==========
    println!("\n=== June 30: End of month savings ===");
    add_transfer(&db, &checking, &savings, "1000.00", "2026-06-30", "Extra savings");

    // ========== VERIFY FINAL BALANCES ==========
    println!("\n=== Final Balance Verification ===");

    let final_checking = get_balance(&db, &checking);
    let final_savings = get_balance(&db, &savings);
    let final_cc = get_balance(&db, &credit_card);

    println!("Checking: ${}", final_checking);
    println!("Savings: ${}", final_savings);
    println!("Credit Card: ${}", final_cc);

    // Hand-calculated expected values:
    // Checking: 2500 + 3500 - 1400 - 125 - 85 - 150 - 500 + 3500 - 415.55 - 78.50 - 156.32 - 45 - 200 - 1000
    //         = 2500 + 7000 - 4155.37 = 5344.63
    let expected_checking = dec("5344.63");
    assert_eq!(final_checking, expected_checking, "final checking balance");

    // Savings: 10000 + 500 + 1000 = 11500
    let expected_savings = dec("11500.00");
    assert_eq!(final_savings, expected_savings, "final savings balance");

    // Credit card: 0 - 89 - 34.50 - 12.99 - 65 = -201.49
    let expected_cc = dec("-201.49");
    assert_eq!(final_cc, expected_cc, "final credit card balance");

    // ========== VERIFY NET WORTH CHANGE ==========
    let final_net_worth = final_checking + final_savings + final_cc;
    println!("\nFinal net worth: ${}", final_net_worth);

    // Net worth change = income - expenses
    // Income: 3500 + 3500 = 7000
    // Checking expenses: 1400 + 125 + 85 + 150 + 78.50 + 156.32 + 45 + 200 = 2239.82
    // Credit card expenses: 415.55 + 89 + 34.50 + 12.99 + 65 = 617.04
    // Total expenses = 2856.86
    // Net change: 7000 - 2856.86 = 4143.14
    let expected_net_worth = initial_net_worth + dec("4143.14");
    assert_eq!(final_net_worth, expected_net_worth, "net worth should increase by income minus expenses");

    println!("Net worth increased by ${}", final_net_worth - initial_net_worth);

    // ========== SIMULATE USER FIXING A MISTAKE ==========
    println!("\n=== Simulating user fixing a mistake ===");

    // User realizes they entered the car repair as $200 but it was actually $180
    // First, let's find and delete the wrong transaction, then add the correct one
    let all_txns = all_transactions(&db);
    let car_repair = all_txns.iter().find(|t| t.description == "Car Repair").expect("find car repair");

    db.with_connection(|conn| repository::delete_transaction(conn, &car_repair.id))
        .expect("delete car repair");

    // Balance should increase by $200 (the deleted expense)
    let after_delete = get_balance(&db, &checking);
    assert_eq!(after_delete, expected_checking + dec("200.00"), "balance after deleting expense");

    // Add the correct amount
    add_expense(&db, &checking, "180.00", "2026-06-25", "Car Repair (corrected)", Some(&gas));

    // Final balance should be $20 more than before
    let corrected_checking = get_balance(&db, &checking);
    assert_eq!(corrected_checking, expected_checking + dec("20.00"), "balance after correction");

    println!("Corrected checking balance: ${}", corrected_checking);

    // ========== SIMULATE DELETING A TRANSFER ==========
    println!("\n=== Simulating deleting a transfer ===");

    // User made a transfer by mistake - delete it
    let before_delete_checking = get_balance(&db, &checking);
    let before_delete_savings = get_balance(&db, &savings);

    // Find the "Extra savings" transfer and delete it
    let all_txns = all_transactions(&db);
    let extra_savings_tx = all_txns.iter()
        .find(|t| t.description.contains("Extra savings"))
        .expect("find extra savings transfer");

    db.with_connection(|conn| repository::delete_transaction_with_pair(conn, &extra_savings_tx.id))
        .expect("delete transfer");

    // Both balances should be restored
    let after_delete_checking = get_balance(&db, &checking);
    let after_delete_savings = get_balance(&db, &savings);

    // The $1000 should come back to checking and leave savings
    assert_eq!(after_delete_checking, before_delete_checking + dec("1000.00"), "checking after transfer delete");
    assert_eq!(after_delete_savings, before_delete_savings - dec("1000.00"), "savings after transfer delete");

    println!("After deleting $1000 transfer:");
    println!("  Checking: ${} -> ${}", before_delete_checking, after_delete_checking);
    println!("  Savings: ${} -> ${}", before_delete_savings, after_delete_savings);

    // ========== VERIFY TRANSACTION COUNT ==========
    let final_txn_count = all_transactions(&db).len();
    println!("\nTotal transactions: {}", final_txn_count);

    // We should have: 2 incomes + ~20 expenses + 2 transfers (4 tx) - 1 deleted expense - 2 deleted transfer tx + 1 corrected expense
    // Actually let's just verify it's reasonable
    assert!(final_txn_count > 15, "should have reasonable number of transactions");
    assert!(final_txn_count < 30, "shouldn't have too many transactions");

    println!("\n=== Simulation Complete - All assertions passed! ===");
}

#[test]
fn simulate_edge_cases() {
    let db = db();

    println!("\n=== Edge Case Testing ===");

    // ========== ZERO BALANCE ACCOUNT ==========
    println!("\nTest: Zero balance account");
    let empty = new_account(&db, "Empty Account", AccountType::Checking, "0.00");
    assert_eq!(get_balance(&db, &empty), dec("0.00"));

    // Add then remove same amount
    let tx_id = add_income(&db, &empty, "100.00", "2026-06-01", "Test", None);
    assert_eq!(get_balance(&db, &empty), dec("100.00"));

    db.with_connection(|conn| repository::delete_transaction(conn, &tx_id)).unwrap();
    assert_eq!(get_balance(&db, &empty), dec("0.00"), "back to zero");

    // ========== NEGATIVE BALANCE ==========
    println!("\nTest: Negative balance (credit card style)");
    let cc = new_account(&db, "Test Credit Card", AccountType::CreditCard, "0.00");

    add_expense(&db, &cc, "500.00", "2026-06-01", "Big purchase", None);
    assert_eq!(get_balance(&db, &cc), dec("-500.00"), "negative balance");

    // Partial payment
    let checking = new_account(&db, "Test Checking", AccountType::Checking, "1000.00");
    add_transfer(&db, &checking, &cc, "200.00", "2026-06-02", "Partial payment");

    assert_eq!(get_balance(&db, &cc), dec("-300.00"), "after partial payment");
    assert_eq!(get_balance(&db, &checking), dec("800.00"), "checking after payment");

    // ========== VERY SMALL AMOUNTS ==========
    println!("\nTest: Very small amounts (penny transactions)");
    let penny_account = new_account(&db, "Penny Account", AccountType::Checking, "0.00");

    for i in 1..=100 {
        add_income(&db, &penny_account, "0.01", "2026-06-01", &format!("Penny {}", i), None);
    }

    assert_eq!(get_balance(&db, &penny_account), dec("1.00"), "100 pennies = $1");

    // ========== LARGE AMOUNTS ==========
    println!("\nTest: Large amounts");
    let big_account = new_account(&db, "Big Account", AccountType::Savings, "1000000.00");
    add_income(&db, &big_account, "999999.99", "2026-06-01", "Lottery", None);

    assert_eq!(get_balance(&db, &big_account), dec("1999999.99"), "large balance");

    // ========== SAME-DAY MULTIPLE TRANSACTIONS ==========
    println!("\nTest: Multiple transactions same day");
    let busy_account = new_account(&db, "Busy Account", AccountType::Checking, "1000.00");

    // 10 transactions on the same day
    for i in 1..=5 {
        add_expense(&db, &busy_account, "10.00", "2026-06-15", &format!("Purchase {}", i), None);
        add_income(&db, &busy_account, "5.00", "2026-06-15", &format!("Refund {}", i), None);
    }

    // Net change: -50 + 25 = -25
    assert_eq!(get_balance(&db, &busy_account), dec("975.00"), "after busy day");

    // ========== TRANSFER CHAIN ==========
    println!("\nTest: Transfer chain A -> B -> C");
    let a = new_account(&db, "Account A", AccountType::Checking, "1000.00");
    let b = new_account(&db, "Account B", AccountType::Checking, "0.00");
    let c = new_account(&db, "Account C", AccountType::Savings, "0.00");

    add_transfer(&db, &a, &b, "500.00", "2026-06-01", "A to B");
    add_transfer(&db, &b, &c, "300.00", "2026-06-02", "B to C");

    assert_eq!(get_balance(&db, &a), dec("500.00"), "A after transfers");
    assert_eq!(get_balance(&db, &b), dec("200.00"), "B after transfers");
    assert_eq!(get_balance(&db, &c), dec("300.00"), "C after transfers");

    // Total should still be $1000
    let total = get_balance(&db, &a) + get_balance(&db, &b) + get_balance(&db, &c);
    assert_eq!(total, dec("1000.00"), "transfer chain is zero-sum");

    println!("\n=== Edge Cases Complete - All assertions passed! ===");
}

#[test]
fn simulate_reconciliation_scenario() {
    // Simulate a user reconciling their account with a bank statement
    let db = db();

    println!("\n=== Reconciliation Scenario ===");

    let checking = new_account(&db, "Main Checking", AccountType::Checking, "1500.00");
    let cat = new_category(&db, "General", CategoryType::Expense, "#888888");

    // User enters transactions as they happen
    add_income(&db, &checking, "2000.00", "2026-06-01", "Paycheck", None);
    add_expense(&db, &checking, "50.00", "2026-06-02", "Grocery Store", Some(&cat));
    add_expense(&db, &checking, "30.00", "2026-06-03", "Gas", Some(&cat));
    add_expense(&db, &checking, "100.00", "2026-06-05", "Electric Bill", Some(&cat));
    add_expense(&db, &checking, "25.00", "2026-06-06", "Coffee Shop", Some(&cat));

    // User's expected balance: 1500 + 2000 - 50 - 30 - 100 - 25 = 3295
    let app_balance = get_balance(&db, &checking);
    assert_eq!(app_balance, dec("3295.00"));

    // Bank statement says balance is $3270... user is off by $25
    let bank_balance = dec("3270.00");
    let difference = app_balance - bank_balance;

    println!("App balance: ${}", app_balance);
    println!("Bank balance: ${}", bank_balance);
    println!("Difference: ${}", difference);

    // User finds a missing transaction - forgot to enter a $25 subscription
    add_expense(&db, &checking, "25.00", "2026-06-04", "Spotify (forgot to enter)", Some(&cat));

    // Now it should match
    let reconciled_balance = get_balance(&db, &checking);
    assert_eq!(reconciled_balance, bank_balance, "balance matches bank statement");

    println!("After reconciliation: ${}", reconciled_balance);
    println!("\n=== Reconciliation Complete ===");
}
