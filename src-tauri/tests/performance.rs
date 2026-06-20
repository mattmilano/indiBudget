//! Performance test — verify derived balances scale to realistic data volumes.
//!
//! Tests with 1000+ transactions to ensure balance computation doesn't become
//! a bottleneck. A real user might have several years of transaction history.

mod common;

use common::*;
use indibudget_lib::database::repository;
use indibudget_lib::models::*;
use std::time::Instant;

#[test]
fn performance_1000_transactions() {
    let db = db();

    let checking = new_account(&db, "Checking", AccountType::Checking, "5000.00");
    let savings = new_account(&db, "Savings", AccountType::Savings, "10000.00");
    let credit = new_account(&db, "Credit Card", AccountType::CreditCard, "0.00");

    println!("\n=== Performance Test: 1000 Transactions ===");

    // Create 1000 transactions across 3 accounts
    let start = Instant::now();

    db.with_connection(|conn| {
        for i in 0..1000 {
            let day = (i % 28) + 1;
            let month = (i / 28 % 12) + 1;
            let year = 2024 + (i / 336);
            let date_str = format!("{:04}-{:02}-{:02}", year, month, day);

            let tx_type = match i % 10 {
                0..=1 => TransactionType::Income,
                2..=8 => TransactionType::Expense,
                _ => TransactionType::Transfer,
            };

            let account_id = match i % 3 {
                0 => &checking,
                1 => &savings,
                _ => &credit,
            };

            let amount = dec(&format!("{}.{:02}", 10 + (i % 100), i % 100));

            let mut tx = Transaction::new(
                account_id.to_string(),
                tx_type.clone(),
                amount,
                date(&date_str),
                format!("Transaction {}", i),
            );

            // For transfers, set up proper description format
            if tx_type == TransactionType::Transfer {
                if i % 2 == 0 {
                    tx.description = format!("Transfer to Savings {}", i);
                } else {
                    tx.description = format!("Transfer from Checking {}", i);
                }
            }

            repository::create_transaction(conn, &tx)?;
        }
        Ok(())
    })
    .expect("create transactions");

    let insert_time = start.elapsed();
    println!("Insert 1000 transactions: {:?}", insert_time);

    // Time balance computation for a single account
    let start = Instant::now();
    let _balance = get_balance(&db, &checking);
    let single_balance_time = start.elapsed();
    println!("Compute single account balance: {:?}", single_balance_time);

    // Time fetching all accounts (which computes all balances)
    let start = Instant::now();
    let accounts = db
        .with_connection(|conn| repository::get_all_accounts(conn))
        .expect("get accounts");
    let all_balances_time = start.elapsed();
    println!("Compute all account balances: {:?}", all_balances_time);

    // Time fetching all transactions
    let start = Instant::now();
    let txns = all_transactions(&db);
    let fetch_txns_time = start.elapsed();
    println!("Fetch all transactions: {:?}", fetch_txns_time);

    // Verify counts
    assert_eq!(accounts.len(), 3);
    assert_eq!(txns.len(), 1000);

    // Performance assertions (generous limits - mainly catching O(n²) bugs)
    assert!(
        single_balance_time.as_millis() < 100,
        "single balance should compute in <100ms, took {:?}",
        single_balance_time
    );
    assert!(
        all_balances_time.as_millis() < 200,
        "all balances should compute in <200ms, took {:?}",
        all_balances_time
    );
    assert!(
        fetch_txns_time.as_millis() < 500,
        "fetching 1000 txns should take <500ms, took {:?}",
        fetch_txns_time
    );

    println!("\n=== Performance Test Passed ===");
}

#[test]
fn performance_stress_test_5000_transactions() {
    let db = db();

    let checking = new_account(&db, "Checking", AccountType::Checking, "10000.00");

    println!("\n=== Stress Test: 5000 Transactions ===");

    let start = Instant::now();

    // Batch insert for speed
    db.with_connection(|conn| {
        for i in 0..5000 {
            let day = (i % 28) + 1;
            let month = (i / 28 % 12) + 1;
            let year = 2020 + (i / 336);
            let date_str = format!("{:04}-{:02}-{:02}", year, month, day);

            let tx_type = if i % 5 == 0 {
                TransactionType::Income
            } else {
                TransactionType::Expense
            };

            // Alternate positive and negative to keep balance reasonable
            let amount = if tx_type == TransactionType::Income {
                dec("100.00")
            } else {
                dec("20.00")
            };

            let tx = Transaction::new(
                checking.clone(),
                tx_type,
                amount,
                date(&date_str),
                format!("Txn {}", i),
            );

            repository::create_transaction(conn, &tx)?;
        }
        Ok(())
    })
    .expect("create transactions");

    let insert_time = start.elapsed();
    println!("Insert 5000 transactions: {:?}", insert_time);

    // Compute balance
    let start = Instant::now();
    let balance = get_balance(&db, &checking);
    let balance_time = start.elapsed();
    println!("Compute balance: {:?}", balance_time);

    // Expected: 10000 + (1000 * 100) - (4000 * 20) = 10000 + 100000 - 80000 = 30000
    assert_eq!(balance, dec("30000.00"), "balance should be correct");

    // Performance check
    assert!(
        balance_time.as_millis() < 500,
        "5000-txn balance should compute in <500ms, took {:?}",
        balance_time
    );

    println!("\n=== Stress Test Passed ===");
}

#[test]
fn performance_many_accounts() {
    let db = db();

    println!("\n=== Performance Test: 50 Accounts ===");

    // Create 50 accounts
    let mut account_ids = Vec::new();
    for i in 0..50 {
        let id = new_account(
            &db,
            &format!("Account {}", i),
            AccountType::Checking,
            "1000.00",
        );
        account_ids.push(id);
    }

    // Add 20 transactions per account (1000 total)
    db.with_connection(|conn| {
        for (idx, account_id) in account_ids.iter().enumerate() {
            for j in 0..20 {
                let day = (j % 28) + 1;
                let tx = Transaction::new(
                    account_id.clone(),
                    if j % 3 == 0 {
                        TransactionType::Income
                    } else {
                        TransactionType::Expense
                    },
                    dec("50.00"),
                    date(&format!("2026-06-{:02}", day)),
                    format!("Acct {} Txn {}", idx, j),
                );
                repository::create_transaction(conn, &tx)?;
            }
        }
        Ok(())
    })
    .expect("create transactions");

    // Time getting all accounts with computed balances
    let start = Instant::now();
    let accounts = db
        .with_connection(|conn| repository::get_all_accounts(conn))
        .expect("get accounts");
    let time = start.elapsed();

    println!("Fetch 50 accounts with balances: {:?}", time);

    assert_eq!(accounts.len(), 50);

    // Each account: 1000 + (7 * 50) - (13 * 50) = 1000 + 350 - 650 = 700
    for account in &accounts {
        assert_eq!(account.balance, dec("700.00"), "each account balance");
    }

    assert!(
        time.as_millis() < 300,
        "50 accounts should compute in <300ms, took {:?}",
        time
    );

    println!("\n=== Many Accounts Test Passed ===");
}
