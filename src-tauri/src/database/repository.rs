use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use rusqlite::{params, Connection, Row};
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
    Ok(Account {
        id: row.get(0)?,
        name: row.get(1)?,
        account_type: AccountType::from_str(row.get::<_, String>(2)?.as_str()),
        balance: parse_decimal(&row.get::<_, String>(3)?),
        currency: row.get(4)?,
        institution: row.get(5)?,
        account_number_last4: row.get(6)?,
        is_active: row.get::<_, i32>(7)? == 1,
        created_at: parse_datetime(&row.get::<_, String>(8)?),
        updated_at: parse_datetime(&row.get::<_, String>(9)?),
    })
}

fn transaction_from_row(row: &Row) -> rusqlite::Result<Transaction> {
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
        status: TransactionStatus::from_str(row.get::<_, String>(9)?.as_str()),
        is_split: row.get::<_, i32>(10)? == 1,
        parent_transaction_id: row.get(11)?,
        recurring_id: row.get(12)?,
        transfer_account_id: row.get(13)?,
        imported_id: row.get(14)?,
        created_at: parse_datetime(&row.get::<_, String>(15)?),
        updated_at: parse_datetime(&row.get::<_, String>(16)?),
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
        "INSERT INTO accounts (id, name, account_type, balance, currency, institution, account_number_last4, is_active, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            account.id,
            account.name,
            account.account_type.as_str(),
            account.balance.to_string(),
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

pub fn get_account(conn: &Connection, id: &str) -> DbResult<Account> {
    conn.query_row(
        "SELECT id, name, account_type, balance, currency, institution, account_number_last4, is_active, created_at, updated_at
         FROM accounts WHERE id = ?1",
        [id],
        account_from_row,
    )
    .map_err(|_| DatabaseError::NotFound)
}

pub fn get_all_accounts(conn: &Connection) -> DbResult<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, account_type, balance, currency, institution, account_number_last4, is_active, created_at, updated_at
         FROM accounts WHERE is_active = 1 ORDER BY name",
    )?;
    let accounts = stmt.query_map([], account_from_row)?.filter_map(|r| r.ok()).collect();
    Ok(accounts)
}

pub fn update_account(conn: &Connection, account: &Account) -> DbResult<()> {
    conn.execute(
        "UPDATE accounts SET name = ?1, account_type = ?2, balance = ?3, currency = ?4, institution = ?5,
         account_number_last4 = ?6, is_active = ?7, updated_at = ?8 WHERE id = ?9",
        params![
            account.name,
            account.account_type.as_str(),
            account.balance.to_string(),
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
    conn.execute("UPDATE accounts SET is_active = 0, updated_at = ?1 WHERE id = ?2", params![Utc::now().to_rfc3339(), id])?;
    Ok(())
}

// Transaction Repository
pub fn create_transaction(conn: &Connection, tx: &Transaction) -> DbResult<()> {
    conn.execute(
        "INSERT INTO transactions (id, account_id, transaction_type, amount, date, description, category_id, payee, notes,
         status, is_split, parent_transaction_id, recurring_id, transfer_account_id, imported_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
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
            tx.imported_id,
            tx.created_at.to_rfc3339(),
            tx.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

pub fn get_transaction(conn: &Connection, id: &str) -> DbResult<Transaction> {
    conn.query_row(
        "SELECT id, account_id, transaction_type, amount, date, description, category_id, payee, notes,
         status, is_split, parent_transaction_id, recurring_id, transfer_account_id, imported_id, created_at, updated_at
         FROM transactions WHERE id = ?1",
        [id],
        transaction_from_row,
    )
    .map_err(|_| DatabaseError::NotFound)
}

pub fn get_transactions(conn: &Connection, filter: &TransactionFilter) -> DbResult<Vec<Transaction>> {
    // Fetch all transactions and filter in memory
    // This is simpler and avoids SQL parameter binding complexity
    let sql = "SELECT id, account_id, transaction_type, amount, date, description, category_id, payee, notes,
         status, is_split, parent_transaction_id, recurring_id, transfer_account_id, imported_id, created_at, updated_at
         FROM transactions ORDER BY date DESC, created_at DESC";

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| transaction_from_row(row))?;

    let transactions: Vec<Transaction> = rows
        .filter_map(|r| r.ok())
        .filter(|tx| {
            if let Some(ref account_ids) = filter.account_ids {
                if !account_ids.contains(&tx.account_id) {
                    return false;
                }
            }
            if let Some(ref category_ids) = filter.category_ids {
                if let Some(ref cat_id) = tx.category_id {
                    if !category_ids.contains(cat_id) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            if let Some(start) = filter.start_date {
                if tx.date < start {
                    return false;
                }
            }
            if let Some(end) = filter.end_date {
                if tx.date > end {
                    return false;
                }
            }
            if let Some(ref search) = filter.search_text {
                let search_lower = search.to_lowercase();
                if !tx.description.to_lowercase().contains(&search_lower)
                    && !tx.payee.as_ref().map_or(false, |p| p.to_lowercase().contains(&search_lower))
                {
                    return false;
                }
            }
            true
        })
        .collect();

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
    let categories = stmt.query_map([], category_from_row)?.filter_map(|r| r.ok()).collect();
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
    let budgets = stmt.query_map([], budget_from_row)?.filter_map(|r| r.ok()).collect();
    Ok(budgets)
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
    let recurring = stmt.query_map([], recurring_from_row)?.filter_map(|r| r.ok()).collect();
    Ok(recurring)
}

pub fn update_recurring_next_occurrence(conn: &Connection, id: &str, next: NaiveDate) -> DbResult<()> {
    conn.execute(
        "UPDATE recurring_transactions SET next_occurrence = ?1, updated_at = ?2 WHERE id = ?3",
        params![next.format("%Y-%m-%d").to_string(), Utc::now().to_rfc3339(), id],
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

pub fn create_cancelled_subscription(conn: &Connection, cancelled: &CancelledSubscription) -> DbResult<()> {
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
    let goals = stmt.query_map([], goal_from_row)?.filter_map(|r| r.ok()).collect();
    Ok(goals)
}

pub fn update_goal_amount(conn: &Connection, id: &str, amount: Decimal) -> DbResult<()> {
    conn.execute(
        "UPDATE savings_goals SET current_amount = ?1, updated_at = ?2 WHERE id = ?3",
        params![amount.to_string(), Utc::now().to_rfc3339(), id],
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
