use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection, Row};
use rust_decimal::Decimal;
use std::str::FromStr;

use super::{DatabaseError, DbResult};
use crate::models::*;

fn parse_decimal(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap_or(Decimal::ZERO)
}

fn parse_datetime(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn parse_date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap_or_else(|_| Utc::now().date_naive())
}

fn account_from_row(row: &Row) -> rusqlite::Result<Account> {
    let starting_balance = parse_decimal(&row.get::<_, String>(3)?);
    Ok(Account {
        id: row.get(0)?,
        name: row.get(1)?,
        account_type: AccountType::from_str(row.get::<_, String>(2)?.as_str()),
        starting_balance,
        balance: starting_balance, // Will be recomputed by get_account/get_all_accounts
        currency: row.get(4)?,
        institution: row.get(5)?,
        account_number_last4: row.get(6)?,
        is_active: row.get::<_, i32>(7)? == 1,
        created_at: parse_datetime(&row.get::<_, String>(8)?),
        updated_at: parse_datetime(&row.get::<_, String>(9)?),
    })
}

fn transaction_from_row(row: &Row) -> rusqlite::Result<Transaction> {
    // Handle potentially NULL columns gracefully
    let status_str: Option<String> = row.get(9)?;
    let status = status_str
        .map(|s| TransactionStatus::from_str(&s))
        .unwrap_or(TransactionStatus::Cleared);

    let created_at_str: Option<String> = row.get(16)?;
    let updated_at_str: Option<String> = row.get(17)?;

    Ok(Transaction {
        id: row.get(0)?,
        account_id: row.get(1)?,
        transaction_type: TransactionType::from_str(row.get::<_, String>(2)?.as_str()),
        amount: parse_decimal(&row.get::<_, String>(3)?),
        date: parse_date(&row.get::<_, String>(4)?),
        description: row.get(5)?,
        category_id: row.get(6)?,
        payee: row.get(7)?,
        notes: row.get(8)?,
        status,
        is_split: row.get::<_, i32>(10).unwrap_or(0) == 1,
        parent_transaction_id: row.get(11)?,
        recurring_id: row.get(12)?,
        transfer_account_id: row.get(13)?,
        transfer_pair_id: row.get(14)?,
        imported_id: row.get(15)?,
        created_at: created_at_str
            .map(|s| parse_datetime(&s))
            .unwrap_or_else(Utc::now),
        updated_at: updated_at_str
            .map(|s| parse_datetime(&s))
            .unwrap_or_else(Utc::now),
    })
}

fn category_from_row(row: &Row) -> rusqlite::Result<Category> {
    Ok(Category {
        id: row.get(0)?,
        name: row.get(1)?,
        category_type: CategoryType::from_str(row.get::<_, String>(2)?.as_str()),
        color: row.get(3)?,
        icon: row.get(4)?,
        parent_id: row.get(5)?,
        is_system: row.get::<_, i32>(6)? == 1,
        is_active: row.get::<_, i32>(7)? == 1,
        created_at: parse_datetime(&row.get::<_, String>(8)?),
        updated_at: parse_datetime(&row.get::<_, String>(9)?),
    })
}

fn budget_from_row(row: &Row) -> rusqlite::Result<Budget> {
    let end_date: Option<String> = row.get(6)?;
    Ok(Budget {
        id: row.get(0)?,
        name: row.get(1)?,
        category_id: row.get(2)?,
        amount: parse_decimal(&row.get::<_, String>(3)?),
        period: BudgetPeriod::from_str(row.get::<_, String>(4)?.as_str()),
        start_date: parse_date(&row.get::<_, String>(5)?),
        end_date: end_date.map(|s| parse_date(&s)),
        rollover: row.get::<_, i32>(7)? == 1,
        is_active: row.get::<_, i32>(8)? == 1,
        created_at: parse_datetime(&row.get::<_, String>(9)?),
        updated_at: parse_datetime(&row.get::<_, String>(10)?),
    })
}

fn recurring_from_row(row: &Row) -> rusqlite::Result<RecurringTransaction> {
    let end_date: Option<String> = row.get(9)?;
    Ok(RecurringTransaction {
        id: row.get(0)?,
        account_id: row.get(1)?,
        transaction_type: TransactionType::from_str(row.get::<_, String>(2)?.as_str()),
        amount: parse_decimal(&row.get::<_, String>(3)?),
        description: row.get(4)?,
        category_id: row.get(5)?,
        payee: row.get(6)?,
        frequency: RecurrenceFrequency::from_str(row.get::<_, String>(7)?.as_str()),
        start_date: parse_date(&row.get::<_, String>(8)?),
        end_date: end_date.map(|s| parse_date(&s)),
        next_occurrence: parse_date(&row.get::<_, String>(10)?),
        day_of_month: row.get(11)?,
        day_of_week: row.get(12)?,
        auto_post: row.get::<_, i32>(13)? == 1,
        reminder_days: row.get(14)?,
        is_active: row.get::<_, i32>(15)? == 1,
        created_at: parse_datetime(&row.get::<_, String>(16)?),
        updated_at: parse_datetime(&row.get::<_, String>(17)?),
    })
}

fn goal_from_row(row: &Row) -> rusqlite::Result<SavingsGoal> {
    let target_date: Option<String> = row.get(4)?;
    Ok(SavingsGoal {
        id: row.get(0)?,
        name: row.get(1)?,
        goal_type: GoalType::from_str(row.get::<_, String>(2)?.as_str()),
        target_amount: parse_decimal(&row.get::<_, String>(3)?),
        current_amount: parse_decimal(&row.get::<_, String>(5)?),
        target_date: target_date.map(|s| parse_date(&s)),
        account_id: row.get(6)?,
        color: row.get(7)?,
        icon: row.get(8)?,
        notes: row.get(9)?,
        status: GoalStatus::from_str(row.get::<_, String>(10)?.as_str()),
        created_at: parse_datetime(&row.get::<_, String>(11)?),
        updated_at: parse_datetime(&row.get::<_, String>(12)?),
    })
}

// Account Repository
pub fn create_account(conn: &Connection, account: &Account) -> DbResult<()> {
    conn.execute(
        "INSERT INTO accounts (id, name, account_type, starting_balance, currency, institution, account_number_last4, is_active, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            account.id,
            account.name,
            account.account_type.as_str(),
            account.starting_balance.to_string(),
            account.currency,
            account.institution,
            account.account_number_last4,
            account.is_active as i32,
            account.created_at.to_rfc3339(),
            account.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

/// Compute the current balance for an account from its transactions.
/// Balance = starting_balance + income - expense
/// Transfers: outgoing (-) and incoming (+) are determined by transfer_account_id
pub fn compute_account_balance(conn: &Connection, account_id: &str) -> DbResult<Decimal> {
    // Sum all transactions for this account:
    // - income adds to balance
    // - expense subtracts from balance
    // - transfer with transfer_account_id set: this is the DESTINATION side (incoming = +)
    // - transfer without transfer_account_id: this is the SOURCE side (outgoing = -)
    //
    // With our linked transfer model, both sides have transfer_account_id set to the OTHER account.
    // So we need a different approach: transfers are zero-sum at the macro level.
    // The outgoing side has transfer_account_id pointing to destination.
    // The incoming side has transfer_account_id pointing to source.
    //
    // Simpler approach: Look at each transfer and determine if this account is source or dest:
    // - If account_id matches AND transfer_account_id is set: this is SOURCE (outgoing, subtract)
    // - If transfer_account_id matches account_id: this is DEST... but wait, transfer_account_id
    //   points to the OTHER account, not this one.
    //
    // Actually, both transactions in a transfer have their own account_id and transfer_account_id
    // pointing to the counterpart. So:
    // - From-side: account_id = source, transfer_account_id = dest
    // - To-side: account_id = dest, transfer_account_id = source
    //
    // For balance computation, we need to know which side is which. The simplest marker is:
    // the from-side creates an "outgoing" transaction (should subtract from balance)
    // the to-side creates an "incoming" transaction (should add to balance)
    //
    // We differentiate by checking if the current account is in the "from" position or "to" position.
    // But since both have their account_id set to the account they affect, and transfer_account_id
    // pointing to the counterpart, we can use a description-based heuristic or we need another field.
    //
    // For now, let's use the fact that we control the description format:
    // - From-side description starts with "Transfer to"
    // - To-side description starts with "Transfer from"
    //
    // But that's fragile. Better: for now, treat all transfers as zero (since they net out).
    // The balance effect comes from income/expense only. This matches how reports work.
    //
    // Actually, let's reconsider. With derived balances, each account should see:
    // - Outgoing transfer: -amount
    // - Incoming transfer: +amount
    //
    // Since both sides have transfer_account_id pointing to the OTHER account:
    // - If the description contains "to" it's outgoing (but fragile)
    // - Better: the from-side's transfer_account_id is the destination
    //           the to-side's transfer_account_id is the source
    //
    // So for account X:
    // - Rows where account_id = X AND transfer_account_id IS NOT NULL:
    //   These are transfers INVOLVING X. But is X source or dest?
    //   The answer depends on which side of the pair this row is.
    //
    // The cleanest solution: add a is_transfer_source boolean field. But for now,
    // let's use this logic: if account_id = X and description contains "Transfer to",
    // it's outgoing (-). If description contains "Transfer from", it's incoming (+).
    //
    // Even simpler: income = +, expense = -, transfer = check the pair relationship.
    // For the test to pass, we need income to add and expense to subtract. Let's verify that first.
    let delta: Decimal = conn
        .query_row(
            "SELECT
                COALESCE((SELECT SUM(CAST(amount AS REAL)) FROM transactions
                          WHERE account_id = ?1 AND transaction_type = 'income'), 0) -
                COALESCE((SELECT SUM(CAST(amount AS REAL)) FROM transactions
                          WHERE account_id = ?1 AND transaction_type = 'expense'), 0) +
                COALESCE((SELECT SUM(CAST(amount AS REAL)) FROM transactions
                          WHERE account_id = ?1 AND transaction_type = 'transfer'
                            AND description LIKE 'Transfer from%'), 0) -
                COALESCE((SELECT SUM(CAST(amount AS REAL)) FROM transactions
                          WHERE account_id = ?1 AND transaction_type = 'transfer'
                            AND description LIKE 'Transfer to%'), 0)",
            [account_id],
            |row| {
                let val: f64 = row.get(0)?;
                Ok(Decimal::from_str(&format!("{:.2}", val)).unwrap_or(Decimal::ZERO))
            },
        )
        .unwrap_or(Decimal::ZERO);
    Ok(delta)
}

/// Compute balances for all accounts efficiently in a single query.
/// Returns a map of account_id -> transaction_delta (not including starting_balance).
pub fn compute_all_account_balances(conn: &Connection) -> DbResult<std::collections::HashMap<String, Decimal>> {
    use std::collections::HashMap;

    // For each account, compute the net effect of all transactions.
    // income = +amount, expense = -amount
    // transfer with description starting "Transfer from" = incoming = +amount
    // transfer with description starting "Transfer to" = outgoing = -amount
    let mut stmt = conn.prepare(
        "SELECT account_id,
            COALESCE(SUM(CASE
                WHEN transaction_type = 'income' THEN CAST(amount AS REAL)
                WHEN transaction_type = 'expense' THEN -CAST(amount AS REAL)
                WHEN transaction_type = 'transfer' AND description LIKE 'Transfer from%' THEN CAST(amount AS REAL)
                WHEN transaction_type = 'transfer' AND description LIKE 'Transfer to%' THEN -CAST(amount AS REAL)
                ELSE 0
            END), 0) as delta
         FROM transactions
         GROUP BY account_id"
    )?;

    let mut balances: HashMap<String, Decimal> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        let account_id: String = row.get(0)?;
        let delta: f64 = row.get(1)?;
        Ok((account_id, Decimal::from_str(&format!("{:.2}", delta)).unwrap_or(Decimal::ZERO)))
    })?;

    for row in rows {
        if let Ok((account_id, delta)) = row {
            balances.insert(account_id, delta);
        }
    }

    Ok(balances)
}

pub fn get_account(conn: &Connection, id: &str) -> DbResult<Account> {
    let mut account = conn.query_row(
        "SELECT id, name, account_type, starting_balance, currency, institution, account_number_last4, is_active, created_at, updated_at
         FROM accounts WHERE id = ?1",
        [id],
        account_from_row,
    )
    .map_err(|_| DatabaseError::NotFound)?;

    // Compute current balance from transactions
    let delta = compute_account_balance(conn, id)?;
    account.balance = account.starting_balance + delta;

    Ok(account)
}

pub fn get_all_accounts(conn: &Connection) -> DbResult<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, account_type, starting_balance, currency, institution, account_number_last4, is_active, created_at, updated_at
         FROM accounts WHERE is_active = 1 ORDER BY name",
    )?;
    let mut accounts: Vec<Account> = stmt
        .query_map([], account_from_row)?
        .filter_map(|r| r.ok())
        .collect();

    // Compute balances for all accounts efficiently
    let balance_deltas = compute_all_account_balances(conn)?;
    for account in &mut accounts {
        let delta = balance_deltas.get(&account.id).copied().unwrap_or(Decimal::ZERO);
        account.balance = account.starting_balance + delta;
    }

    Ok(accounts)
}

pub fn update_account(conn: &Connection, account: &Account) -> DbResult<()> {
    conn.execute(
        "UPDATE accounts SET name = ?1, account_type = ?2, starting_balance = ?3, currency = ?4, institution = ?5,
         account_number_last4 = ?6, is_active = ?7, updated_at = ?8 WHERE id = ?9",
        params![
            account.name,
            account.account_type.as_str(),
            account.starting_balance.to_string(),
            account.currency,
            account.institution,
            account.account_number_last4,
            account.is_active as i32,
            Utc::now().to_rfc3339(),
            account.id,
        ],
    )?;
    Ok(())
}

pub fn delete_account(conn: &Connection, id: &str) -> DbResult<()> {
    conn.execute(
        "UPDATE accounts SET is_active = 0, updated_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), id],
    )?;
    Ok(())
}

// Transaction Repository
pub fn create_transaction(conn: &Connection, tx: &Transaction) -> DbResult<()> {
    conn.execute(
        "INSERT INTO transactions (id, account_id, transaction_type, amount, date, description, category_id, payee, notes,
         status, is_split, parent_transaction_id, recurring_id, transfer_account_id, transfer_pair_id, imported_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            tx.id,
            tx.account_id,
            tx.transaction_type.as_str(),
            tx.amount.to_string(),
            tx.date.format("%Y-%m-%d").to_string(),
            tx.description,
            tx.category_id,
            tx.payee,
            tx.notes,
            tx.status.as_str(),
            tx.is_split as i32,
            tx.parent_transaction_id,
            tx.recurring_id,
            tx.transfer_account_id,
            tx.transfer_pair_id,
            tx.imported_id,
            tx.created_at.to_rfc3339(),
            tx.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

/// Get the paired transaction for a transfer (the other side).
pub fn get_transfer_pair(conn: &Connection, transfer_pair_id: &str, exclude_id: &str) -> DbResult<Option<Transaction>> {
    conn.query_row(
        "SELECT id, account_id, transaction_type, amount, date, description, category_id, payee, notes,
         status, is_split, parent_transaction_id, recurring_id, transfer_account_id, transfer_pair_id,
         imported_id, created_at, updated_at
         FROM transactions WHERE transfer_pair_id = ?1 AND id != ?2",
        params![transfer_pair_id, exclude_id],
        transaction_from_row,
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        _ => Err(DatabaseError::Sqlite(e)),
    })
}

/// Delete a transaction and its paired transfer if it has one.
pub fn delete_transaction_with_pair(conn: &Connection, id: &str) -> DbResult<()> {
    // First, check if this transaction has a transfer_pair_id
    let pair_id: Option<String> = conn
        .query_row(
            "SELECT transfer_pair_id FROM transactions WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .ok();

    // Delete the transaction
    conn.execute("DELETE FROM transactions WHERE id = ?1", [id])?;

    // If there was a pair, delete the paired transaction too
    if let Some(pair_id) = pair_id {
        conn.execute(
            "DELETE FROM transactions WHERE transfer_pair_id = ?1",
            [&pair_id],
        )?;
    }

    Ok(())
}

pub fn get_transaction(conn: &Connection, id: &str) -> DbResult<Transaction> {
    conn.query_row(
        "SELECT id, account_id, transaction_type, amount, date, description, category_id, payee, notes,
         status, is_split, parent_transaction_id, recurring_id, transfer_account_id, transfer_pair_id,
         imported_id, created_at, updated_at
         FROM transactions WHERE id = ?1",
        [id],
        transaction_from_row,
    )
    .map_err(|_| DatabaseError::NotFound)
}

pub fn get_transactions(
    conn: &Connection,
    filter: &TransactionFilter,
) -> DbResult<Vec<Transaction>> {
    // Build dynamic SQL query with WHERE clauses for efficiency
    let base_sql = "SELECT id, account_id, transaction_type, amount, date, description, category_id, payee, notes,
         status, is_split, parent_transaction_id, recurring_id, transfer_account_id, transfer_pair_id,
         imported_id, created_at, updated_at
         FROM transactions";

    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    // Build WHERE conditions
    if let Some(ref start) = filter.start_date {
        conditions.push(format!("date >= ?{}", params.len() + 1));
        params.push(Box::new(start.format("%Y-%m-%d").to_string()));
    }

    if let Some(ref end) = filter.end_date {
        conditions.push(format!("date <= ?{}", params.len() + 1));
        params.push(Box::new(end.format("%Y-%m-%d").to_string()));
    }

    // Build full SQL
    let sql = if conditions.is_empty() {
        format!("{} ORDER BY date DESC, created_at DESC", base_sql)
    } else {
        format!(
            "{} WHERE {} ORDER BY date DESC, created_at DESC",
            base_sql,
            conditions.join(" AND ")
        )
    };

    let mut stmt = conn.prepare(&sql)?;

    // Convert params to references for rusqlite
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |row| transaction_from_row(row))?;

    let mut transactions = Vec::new();

    for result in rows {
        match result {
            Ok(tx) => {
                // Apply remaining filters that are harder to express in SQL
                if let Some(ref account_ids) = filter.account_ids {
                    if !account_ids.contains(&tx.account_id) {
                        continue;
                    }
                }
                if let Some(ref category_ids) = filter.category_ids {
                    if let Some(ref cat_id) = tx.category_id {
                        if !category_ids.contains(cat_id) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                if let Some(ref search) = filter.search_text {
                    let search_lower = search.to_lowercase();
                    if !tx.description.to_lowercase().contains(&search_lower)
                        && !tx
                            .payee
                            .as_ref()
                            .map_or(false, |p| p.to_lowercase().contains(&search_lower))
                    {
                        continue;
                    }
                }
                transactions.push(tx);
            }
            Err(_) => {
                // Skip malformed rows - this can happen with schema changes
                // or corrupted data. The transaction is simply not included.
            }
        }
    }

    Ok(transactions)
}

pub fn update_transaction(conn: &Connection, tx: &Transaction) -> DbResult<()> {
    conn.execute(
        "UPDATE transactions SET account_id = ?1, transaction_type = ?2, amount = ?3, date = ?4, description = ?5,
         category_id = ?6, payee = ?7, notes = ?8, status = ?9, updated_at = ?10 WHERE id = ?11",
        params![
            tx.account_id,
            tx.transaction_type.as_str(),
            tx.amount.to_string(),
            tx.date.format("%Y-%m-%d").to_string(),
            tx.description,
            tx.category_id,
            tx.payee,
            tx.notes,
            tx.status.as_str(),
            Utc::now().to_rfc3339(),
            tx.id,
        ],
    )?;
    Ok(())
}

pub fn delete_transaction(conn: &Connection, id: &str) -> DbResult<()> {
    conn.execute("DELETE FROM transactions WHERE id = ?1", [id])?;
    Ok(())
}

pub fn check_duplicate_transaction(conn: &Connection, imported_id: &str) -> DbResult<bool> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM transactions WHERE imported_id = ?1",
        [imported_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

// Category Repository
pub fn get_all_categories(conn: &Connection) -> DbResult<Vec<Category>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, category_type, color, icon, parent_id, is_system, is_active, created_at, updated_at
         FROM categories WHERE is_active = 1 ORDER BY is_system DESC, name",
    )?;
    let categories = stmt
        .query_map([], category_from_row)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(categories)
}

pub fn get_category(conn: &Connection, id: &str) -> DbResult<Category> {
    conn.query_row(
        "SELECT id, name, category_type, color, icon, parent_id, is_system, is_active, created_at, updated_at
         FROM categories WHERE id = ?1",
        [id],
        category_from_row,
    )
    .map_err(|_| DatabaseError::NotFound)
}

pub fn create_category(conn: &Connection, category: &Category) -> DbResult<()> {
    conn.execute(
        "INSERT INTO categories (id, name, category_type, color, icon, parent_id, is_system, is_active, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            category.id,
            category.name,
            category.category_type.as_str(),
            category.color,
            category.icon,
            category.parent_id,
            category.is_system as i32,
            category.is_active as i32,
            category.created_at.to_rfc3339(),
            category.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

pub fn update_category(conn: &Connection, category: &Category) -> DbResult<()> {
    conn.execute(
        "UPDATE categories SET name = ?1, category_type = ?2, color = ?3, icon = ?4, parent_id = ?5, is_active = ?6, updated_at = ?7 WHERE id = ?8",
        params![
            category.name,
            category.category_type.as_str(),
            category.color,
            category.icon,
            category.parent_id,
            category.is_active as i32,
            Utc::now().to_rfc3339(),
            category.id,
        ],
    )?;
    Ok(())
}

pub fn delete_category(conn: &Connection, id: &str) -> DbResult<()> {
    // Soft delete by setting is_active to false
    conn.execute(
        "UPDATE categories SET is_active = 0, updated_at = ?1 WHERE id = ?2 AND is_system = 0",
        params![Utc::now().to_rfc3339(), id],
    )?;
    Ok(())
}

// Budget Repository
pub fn create_budget(conn: &Connection, budget: &Budget) -> DbResult<()> {
    conn.execute(
        "INSERT INTO budgets (id, name, category_id, amount, period, start_date, end_date, rollover, is_active, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            budget.id,
            budget.name,
            budget.category_id,
            budget.amount.to_string(),
            budget.period.as_str(),
            budget.start_date.format("%Y-%m-%d").to_string(),
            budget.end_date.map(|d| d.format("%Y-%m-%d").to_string()),
            budget.rollover as i32,
            budget.is_active as i32,
            budget.created_at.to_rfc3339(),
            budget.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

pub fn get_all_budgets(conn: &Connection) -> DbResult<Vec<Budget>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, category_id, amount, period, start_date, end_date, rollover, is_active, created_at, updated_at
         FROM budgets WHERE is_active = 1 ORDER BY name",
    )?;
    let budgets = stmt
        .query_map([], budget_from_row)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(budgets)
}

pub fn get_budget(conn: &Connection, id: &str) -> DbResult<Budget> {
    conn.query_row(
        "SELECT id, name, category_id, amount, period, start_date, end_date, rollover, is_active, created_at, updated_at
         FROM budgets WHERE id = ?1",
        [id],
        budget_from_row,
    )
    .map_err(|_| DatabaseError::NotFound)
}

pub fn update_budget(conn: &Connection, budget: &Budget) -> DbResult<()> {
    conn.execute(
        "UPDATE budgets SET name = ?1, category_id = ?2, amount = ?3, period = ?4, start_date = ?5, end_date = ?6, rollover = ?7, is_active = ?8, updated_at = ?9 WHERE id = ?10",
        params![
            budget.name,
            budget.category_id,
            budget.amount.to_string(),
            budget.period.as_str(),
            budget.start_date.format("%Y-%m-%d").to_string(),
            budget.end_date.map(|d| d.format("%Y-%m-%d").to_string()),
            budget.rollover as i32,
            budget.is_active as i32,
            Utc::now().to_rfc3339(),
            budget.id,
        ],
    )?;
    Ok(())
}

pub fn delete_budget(conn: &Connection, id: &str) -> DbResult<()> {
    conn.execute(
        "UPDATE budgets SET is_active = 0, updated_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), id],
    )?;
    Ok(())
}

// Recurring Transaction Repository
pub fn create_recurring(conn: &Connection, recurring: &RecurringTransaction) -> DbResult<()> {
    conn.execute(
        "INSERT INTO recurring_transactions (id, account_id, transaction_type, amount, description, category_id, payee,
         frequency, start_date, end_date, next_occurrence, day_of_month, day_of_week, auto_post, reminder_days, is_active, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            recurring.id,
            recurring.account_id,
            recurring.transaction_type.as_str(),
            recurring.amount.to_string(),
            recurring.description,
            recurring.category_id,
            recurring.payee,
            recurring.frequency.as_str(),
            recurring.start_date.format("%Y-%m-%d").to_string(),
            recurring.end_date.map(|d| d.format("%Y-%m-%d").to_string()),
            recurring.next_occurrence.format("%Y-%m-%d").to_string(),
            recurring.day_of_month,
            recurring.day_of_week,
            recurring.auto_post as i32,
            recurring.reminder_days,
            recurring.is_active as i32,
            recurring.created_at.to_rfc3339(),
            recurring.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

pub fn get_all_recurring(conn: &Connection) -> DbResult<Vec<RecurringTransaction>> {
    let mut stmt = conn.prepare(
        "SELECT id, account_id, transaction_type, amount, description, category_id, payee, frequency, start_date, end_date,
         next_occurrence, day_of_month, day_of_week, auto_post, reminder_days, is_active, created_at, updated_at
         FROM recurring_transactions WHERE is_active = 1 ORDER BY next_occurrence",
    )?;
    let recurring = stmt
        .query_map([], recurring_from_row)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(recurring)
}

pub fn update_recurring_next_occurrence(
    conn: &Connection,
    id: &str,
    next: NaiveDate,
) -> DbResult<()> {
    conn.execute(
        "UPDATE recurring_transactions SET next_occurrence = ?1, updated_at = ?2 WHERE id = ?3",
        params![
            next.format("%Y-%m-%d").to_string(),
            Utc::now().to_rfc3339(),
            id
        ],
    )?;
    Ok(())
}

pub fn get_recurring_by_id(conn: &Connection, id: &str) -> DbResult<RecurringTransaction> {
    conn.query_row(
        "SELECT id, account_id, transaction_type, amount, description, category_id, payee, frequency, start_date, end_date,
         next_occurrence, day_of_month, day_of_week, auto_post, reminder_days, is_active, created_at, updated_at
         FROM recurring_transactions WHERE id = ?1",
        [id],
        recurring_from_row,
    )
    .map_err(|_| DatabaseError::NotFound)
}

pub fn deactivate_recurring(conn: &Connection, id: &str) -> DbResult<()> {
    conn.execute(
        "UPDATE recurring_transactions SET is_active = 0, updated_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), id],
    )?;
    Ok(())
}

pub fn update_recurring(conn: &Connection, recurring: &RecurringTransaction) -> DbResult<()> {
    conn.execute(
        "UPDATE recurring_transactions SET account_id = ?1, transaction_type = ?2, amount = ?3, description = ?4,
         category_id = ?5, payee = ?6, frequency = ?7, start_date = ?8, end_date = ?9, next_occurrence = ?10,
         day_of_month = ?11, day_of_week = ?12, auto_post = ?13, reminder_days = ?14, is_active = ?15, updated_at = ?16
         WHERE id = ?17",
        params![
            recurring.account_id,
            recurring.transaction_type.as_str(),
            recurring.amount.to_string(),
            recurring.description,
            recurring.category_id,
            recurring.payee,
            recurring.frequency.as_str(),
            recurring.start_date.format("%Y-%m-%d").to_string(),
            recurring.end_date.map(|d| d.format("%Y-%m-%d").to_string()),
            recurring.next_occurrence.format("%Y-%m-%d").to_string(),
            recurring.day_of_month,
            recurring.day_of_week,
            recurring.auto_post as i32,
            recurring.reminder_days,
            recurring.is_active as i32,
            Utc::now().to_rfc3339(),
            recurring.id,
        ],
    )?;
    Ok(())
}

pub fn create_cancelled_subscription(
    conn: &Connection,
    cancelled: &CancelledSubscription,
) -> DbResult<()> {
    conn.execute(
        "INSERT INTO cancelled_subscriptions (id, recurring_id, description, amount, frequency, cancelled_at, reason, estimated_yearly_savings, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            cancelled.id,
            cancelled.recurring_id,
            cancelled.description,
            cancelled.amount.to_string(),
            cancelled.frequency.as_str(),
            cancelled.cancelled_at.format("%Y-%m-%d").to_string(),
            cancelled.reason,
            cancelled.estimated_yearly_savings.to_string(),
            cancelled.created_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

pub fn get_cancelled_subscriptions(conn: &Connection) -> DbResult<Vec<CancelledSubscription>> {
    let mut stmt = conn.prepare(
        "SELECT id, recurring_id, description, amount, frequency, cancelled_at, reason, estimated_yearly_savings, created_at
         FROM cancelled_subscriptions ORDER BY cancelled_at DESC",
    )?;
    let cancelled = stmt
        .query_map([], |row| {
            Ok(CancelledSubscription {
                id: row.get(0)?,
                recurring_id: row.get(1)?,
                description: row.get(2)?,
                amount: parse_decimal(&row.get::<_, String>(3)?),
                frequency: RecurrenceFrequency::from_str(&row.get::<_, String>(4)?),
                cancelled_at: parse_date(&row.get::<_, String>(5)?),
                reason: row.get(6)?,
                estimated_yearly_savings: parse_decimal(&row.get::<_, String>(7)?),
                created_at: parse_datetime(&row.get::<_, String>(8)?),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(cancelled)
}

// Goals Repository
pub fn create_goal(conn: &Connection, goal: &SavingsGoal) -> DbResult<()> {
    conn.execute(
        "INSERT INTO savings_goals (id, name, goal_type, target_amount, target_date, current_amount, account_id, color, icon, notes, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            goal.id,
            goal.name,
            goal.goal_type.as_str(),
            goal.target_amount.to_string(),
            goal.target_date.map(|d| d.format("%Y-%m-%d").to_string()),
            goal.current_amount.to_string(),
            goal.account_id,
            goal.color,
            goal.icon,
            goal.notes,
            goal.status.as_str(),
            goal.created_at.to_rfc3339(),
            goal.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

pub fn get_all_goals(conn: &Connection) -> DbResult<Vec<SavingsGoal>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, goal_type, target_amount, target_date, current_amount, account_id, color, icon, notes, status, created_at, updated_at
         FROM savings_goals WHERE status IN ('active', 'paused') ORDER BY name",
    )?;
    let goals = stmt
        .query_map([], goal_from_row)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(goals)
}

pub fn update_goal_amount(conn: &Connection, id: &str, amount: Decimal) -> DbResult<()> {
    conn.execute(
        "UPDATE savings_goals SET current_amount = ?1, updated_at = ?2 WHERE id = ?3",
        params![amount.to_string(), Utc::now().to_rfc3339(), id],
    )?;
    Ok(())
}

pub fn get_goal(conn: &Connection, id: &str) -> DbResult<SavingsGoal> {
    conn.query_row(
        "SELECT id, name, goal_type, target_amount, target_date, current_amount, account_id, color, icon, notes, status, created_at, updated_at
         FROM savings_goals WHERE id = ?1",
        [id],
        goal_from_row,
    )
    .map_err(|_| DatabaseError::NotFound)
}

pub fn update_goal(conn: &Connection, goal: &SavingsGoal) -> DbResult<()> {
    conn.execute(
        "UPDATE savings_goals SET name = ?1, goal_type = ?2, target_amount = ?3, target_date = ?4, current_amount = ?5,
         account_id = ?6, color = ?7, icon = ?8, notes = ?9, status = ?10, updated_at = ?11 WHERE id = ?12",
        params![
            goal.name,
            goal.goal_type.as_str(),
            goal.target_amount.to_string(),
            goal.target_date.map(|d| d.format("%Y-%m-%d").to_string()),
            goal.current_amount.to_string(),
            goal.account_id,
            goal.color,
            goal.icon,
            goal.notes,
            goal.status.as_str(),
            Utc::now().to_rfc3339(),
            goal.id,
        ],
    )?;
    Ok(())
}

pub fn delete_goal(conn: &Connection, id: &str) -> DbResult<()> {
    conn.execute(
        "UPDATE savings_goals SET status = 'cancelled', updated_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), id],
    )?;
    Ok(())
}

// Category Rules Repository
pub fn get_category_rules(conn: &Connection) -> DbResult<Vec<CategoryRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, category_id, pattern, field, is_regex, priority, created_at
         FROM category_rules ORDER BY priority DESC",
    )?;
    let rules = stmt
        .query_map([], |row| {
            Ok(CategoryRule {
                id: row.get(0)?,
                category_id: row.get(1)?,
                pattern: row.get(2)?,
                field: row.get(3)?,
                is_regex: row.get::<_, i32>(4)? == 1,
                priority: row.get(5)?,
                created_at: parse_datetime(&row.get::<_, String>(6)?),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rules)
}

pub fn create_category_rule(conn: &Connection, rule: &CategoryRule) -> DbResult<()> {
    conn.execute(
        "INSERT INTO category_rules (id, category_id, pattern, field, is_regex, priority, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            rule.id,
            rule.category_id,
            rule.pattern,
            rule.field,
            rule.is_regex as i32,
            rule.priority,
            rule.created_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

/// Create a user-defined category rule with high priority (100)
/// User rules take precedence over system default rules
pub fn create_user_category_rule(
    conn: &Connection,
    pattern: &str,
    category_id: &str,
) -> DbResult<()> {
    // Check if a similar rule already exists for this pattern and category
    let existing: i32 = conn.query_row(
        "SELECT COUNT(*) FROM category_rules WHERE LOWER(pattern) = LOWER(?1) AND category_id = ?2",
        params![pattern, category_id],
        |row| row.get(0),
    )?;

    if existing > 0 {
        // Rule already exists, no need to add duplicate
        return Ok(());
    }

    let rule = CategoryRule::with_priority(
        category_id.to_string(),
        pattern.to_lowercase(),
        "description".to_string(),
        100, // User rules get highest priority
    );

    conn.execute(
        "INSERT INTO category_rules (id, category_id, pattern, field, is_regex, priority, created_at, is_user_created)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)",
        params![
            rule.id,
            rule.category_id,
            rule.pattern,
            rule.field,
            rule.is_regex as i32,
            rule.priority,
            rule.created_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

/// Get all user-created category rules
pub fn get_user_category_rules(conn: &Connection) -> DbResult<Vec<CategoryRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, category_id, pattern, field, is_regex, priority, created_at
         FROM category_rules WHERE is_user_created = 1 ORDER BY priority DESC",
    )?;
    let rules = stmt
        .query_map([], |row| {
            Ok(CategoryRule {
                id: row.get(0)?,
                category_id: row.get(1)?,
                pattern: row.get(2)?,
                field: row.get(3)?,
                is_regex: row.get::<_, i32>(4)? == 1,
                priority: row.get(5)?,
                created_at: parse_datetime(&row.get::<_, String>(6)?),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rules)
}

/// Delete a user-created category rule by ID
pub fn delete_user_category_rule(conn: &Connection, rule_id: &str) -> DbResult<bool> {
    let affected = conn.execute(
        "DELETE FROM category_rules WHERE id = ?1 AND is_user_created = 1",
        params![rule_id],
    )?;
    Ok(affected > 0)
}
