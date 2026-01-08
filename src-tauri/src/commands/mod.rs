use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

use crate::database::{self, repository, Database, DbResult};
use crate::models::*;
use crate::services::{self, encryption::EncryptionService, importer, Categorizer};

pub struct AppState {
    pub db: Mutex<Option<Database>>,
    pub encryption: Mutex<Option<EncryptionService>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            db: Mutex::new(None),
            encryption: Mutex::new(None),
        }
    }

    pub fn init_database(&self) -> Result<(), String> {
        let db_path = database::get_database_path();
        let db = Database::new(db_path.clone()).map_err(|e| e.to_string())?;
        *self.db.lock().unwrap() = Some(db);

        // Initialize encryption service
        let data_dir = db_path.parent().unwrap_or(&db_path).to_path_buf();
        let encryption = EncryptionService::new(data_dir).map_err(|e| e.to_string())?;
        *self.encryption.lock().unwrap() = Some(encryption);

        Ok(())
    }
}

fn with_db<F, T>(state: &State<AppState>, f: F) -> Result<T, String>
where
    F: FnOnce(&Database) -> DbResult<T>,
{
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("Database not initialized")?;
    f(db).map_err(|e| e.to_string())
}

// Initialization
#[tauri::command]
pub fn init_app(state: State<AppState>) -> Result<(), String> {
    state.init_database()
}

#[tauri::command]
pub fn get_database_path() -> String {
    database::get_database_path().to_string_lossy().to_string()
}

#[tauri::command]
pub fn get_transaction_count(state: State<AppState>) -> Result<i32, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| {
            let count: i32 = conn
                .query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))
                .map_err(|e| crate::database::DatabaseError::Sqlite(e))?;
            Ok(count)
        })
    })
}

// Account Commands
#[tauri::command]
pub fn create_account(state: State<AppState>, request: CreateAccountRequest) -> Result<Account, String> {
    with_db(&state, |db| {
        let mut account = Account::new(request.name, request.account_type);

        if let Some(balance) = request.balance {
            account.balance = balance;
        }
        if let Some(currency) = request.currency {
            account.currency = currency;
        }
        account.institution = request.institution;
        account.account_number_last4 = request.account_number_last4;

        db.with_connection(|conn| {
            repository::create_account(conn, &account)?;
            Ok(account)
        })
    })
}

#[tauri::command]
pub fn get_accounts(state: State<AppState>) -> Result<Vec<Account>, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| repository::get_all_accounts(conn))
    })
}

#[tauri::command]
pub fn get_account(state: State<AppState>, id: String) -> Result<Account, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| repository::get_account(conn, &id))
    })
}

#[tauri::command]
pub fn update_account(state: State<AppState>, request: UpdateAccountRequest) -> Result<Account, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| {
            let mut account = repository::get_account(conn, &request.id)?;

            if let Some(name) = request.name {
                account.name = name;
            }
            if let Some(account_type) = request.account_type {
                account.account_type = account_type;
            }
            if let Some(balance) = request.balance {
                account.balance = balance;
            }
            if let Some(currency) = request.currency {
                account.currency = currency;
            }
            if let Some(institution) = request.institution {
                account.institution = Some(institution);
            }
            if let Some(last4) = request.account_number_last4 {
                account.account_number_last4 = Some(last4);
            }
            if let Some(is_active) = request.is_active {
                account.is_active = is_active;
            }

            repository::update_account(conn, &account)?;
            Ok(account)
        })
    })
}

#[tauri::command]
pub fn delete_account(state: State<AppState>, id: String) -> Result<(), String> {
    with_db(&state, |db| {
        db.with_connection(|conn| repository::delete_account(conn, &id))
    })
}

// Transaction Commands
#[tauri::command]
pub fn create_transaction(state: State<AppState>, request: CreateTransactionRequest) -> Result<Transaction, String> {
    with_db(&state, |db| {
        let mut tx = Transaction::new(
            request.account_id,
            request.transaction_type,
            request.amount,
            request.date,
            request.description,
        );

        tx.category_id = request.category_id;
        tx.payee = request.payee;
        tx.notes = request.notes;
        tx.status = request.status.unwrap_or(TransactionStatus::Cleared);
        tx.transfer_account_id = request.transfer_account_id;

        db.with_connection(|conn| {
            repository::create_transaction(conn, &tx)?;
            Ok(tx)
        })
    })
}

#[tauri::command]
pub fn get_transactions(state: State<AppState>, filter: TransactionFilter) -> Result<Vec<Transaction>, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| repository::get_transactions(conn, &filter))
    })
}

#[tauri::command]
pub fn get_transaction(state: State<AppState>, id: String) -> Result<Transaction, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| repository::get_transaction(conn, &id))
    })
}

#[tauri::command]
pub fn update_transaction(state: State<AppState>, request: UpdateTransactionRequest) -> Result<Transaction, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| {
            let mut tx = repository::get_transaction(conn, &request.id)?;

            if let Some(account_id) = request.account_id {
                tx.account_id = account_id;
            }
            if let Some(transaction_type) = request.transaction_type {
                tx.transaction_type = transaction_type;
            }
            if let Some(amount) = request.amount {
                tx.amount = amount;
            }
            if let Some(date) = request.date {
                tx.date = date;
            }
            if let Some(description) = request.description {
                tx.description = description;
            }
            if let Some(category_id) = request.category_id {
                tx.category_id = Some(category_id);
            }
            if let Some(payee) = request.payee {
                tx.payee = Some(payee);
            }
            if let Some(notes) = request.notes {
                tx.notes = Some(notes);
            }
            if let Some(status) = request.status {
                tx.status = status;
            }

            repository::update_transaction(conn, &tx)?;
            Ok(tx)
        })
    })
}

#[tauri::command]
pub fn delete_transaction(state: State<AppState>, id: String) -> Result<(), String> {
    with_db(&state, |db| {
        db.with_connection(|conn| repository::delete_transaction(conn, &id))
    })
}

// Category Commands
#[tauri::command]
pub fn get_categories(state: State<AppState>) -> Result<Vec<Category>, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| repository::get_all_categories(conn))
    })
}

#[tauri::command]
pub fn create_category(state: State<AppState>, request: CreateCategoryRequest) -> Result<Category, String> {
    with_db(&state, |db| {
        let mut category = Category::new(request.name, request.category_type, request.color);
        category.icon = request.icon;
        category.parent_id = request.parent_id;

        db.with_connection(|conn| {
            repository::create_category(conn, &category)?;
            Ok(category)
        })
    })
}

// Budget Commands
#[tauri::command]
pub fn create_budget(state: State<AppState>, request: CreateBudgetRequest) -> Result<Budget, String> {
    with_db(&state, |db| {
        let mut budget = Budget::new(
            request.name,
            request.category_id,
            request.amount,
            request.period,
            request.start_date,
        );

        budget.end_date = request.end_date;
        budget.rollover = request.rollover.unwrap_or(false);

        db.with_connection(|conn| {
            repository::create_budget(conn, &budget)?;
            Ok(budget)
        })
    })
}

#[tauri::command]
pub fn get_budgets(state: State<AppState>) -> Result<Vec<Budget>, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| repository::get_all_budgets(conn))
    })
}

#[tauri::command]
pub fn get_budget_status(state: State<AppState>, as_of_date: Option<NaiveDate>) -> Result<Vec<BudgetStatus>, String> {
    let date = as_of_date.unwrap_or_else(|| chrono::Utc::now().date_naive());

    with_db(&state, |db| {
        db.with_connection(|conn| {
            let budgets = repository::get_all_budgets(conn)?;
            let categories = repository::get_all_categories(conn)?;
            let transactions = repository::get_transactions(conn, &TransactionFilter {
                account_ids: None,
                category_ids: None,
                transaction_types: None,
                start_date: None,
                end_date: None,
                min_amount: None,
                max_amount: None,
                search_text: None,
                status: None,
            })?;

            let statuses: Vec<BudgetStatus> = budgets
                .iter()
                .map(|b| services::reports::calculate_budget_status(b, &transactions, &categories, date))
                .collect();

            Ok(statuses)
        })
    })
}

// Recurring Transaction Commands
#[tauri::command]
pub fn create_recurring(state: State<AppState>, request: CreateRecurringRequest) -> Result<RecurringTransaction, String> {
    with_db(&state, |db| {
        let mut recurring = RecurringTransaction::new(
            request.account_id,
            request.transaction_type,
            request.amount,
            request.description,
            request.frequency,
            request.start_date,
        );

        recurring.category_id = request.category_id;
        recurring.payee = request.payee;
        recurring.end_date = request.end_date;
        recurring.day_of_month = request.day_of_month;
        recurring.auto_post = request.auto_post.unwrap_or(false);
        recurring.reminder_days = request.reminder_days;

        db.with_connection(|conn| {
            repository::create_recurring(conn, &recurring)?;
            Ok(recurring)
        })
    })
}

#[tauri::command]
pub fn get_recurring(state: State<AppState>) -> Result<Vec<RecurringTransaction>, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| repository::get_all_recurring(conn))
    })
}

#[tauri::command]
pub fn get_upcoming_recurring(state: State<AppState>, days: Option<i32>) -> Result<Vec<UpcomingRecurring>, String> {
    let days = days.unwrap_or(30);
    let today = chrono::Utc::now().date_naive();
    let end_date = today + chrono::Duration::days(days as i64);

    with_db(&state, |db| {
        db.with_connection(|conn| {
            let recurring = repository::get_all_recurring(conn)?;
            let accounts = repository::get_all_accounts(conn)?;
            let categories = repository::get_all_categories(conn)?;

            let account_map: std::collections::HashMap<String, String> =
                accounts.into_iter().map(|a| (a.id, a.name)).collect();
            let category_map: std::collections::HashMap<String, String> =
                categories.into_iter().map(|c| (c.id, c.name)).collect();

            let mut upcoming: Vec<UpcomingRecurring> = recurring
                .into_iter()
                .filter(|r| r.next_occurrence <= end_date)
                .map(|r| {
                    let days_until = (r.next_occurrence - today).num_days();
                    let category_name = r.category_id.as_ref().and_then(|id| category_map.get(id).cloned());
                    let account_name = account_map.get(&r.account_id).cloned().unwrap_or_default();

                    UpcomingRecurring {
                        next_date: r.next_occurrence,
                        days_until,
                        category_name,
                        account_name,
                        recurring: r,
                    }
                })
                .collect();

            upcoming.sort_by_key(|u| u.next_date);
            Ok(upcoming)
        })
    })
}

#[tauri::command]
pub fn detect_recurring_patterns(state: State<AppState>) -> Result<Vec<services::recurring_detector::DetectedRecurring>, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| {
            // Get all transactions
            let transactions = repository::get_transactions(conn, &TransactionFilter::default())?;

            // Detect recurring patterns
            let detected = services::recurring_detector::detect_recurring_transactions(&transactions);

            Ok(detected)
        })
    })
}

#[tauri::command]
pub fn create_recurring_from_detected(
    state: State<AppState>,
    detected: services::recurring_detector::DetectedRecurring,
) -> Result<RecurringTransaction, String> {
    let today = chrono::Utc::now().date_naive();

    with_db(&state, |db| {
        // Calculate next occurrence based on typical day of month or last occurrence
        let next_occurrence = if let Some(day) = detected.typical_day_of_month {
            // Use the typical day of month, in the current or next month
            let current_month_date = today.with_day(day.min(28)).unwrap_or(today);
            if current_month_date > today {
                current_month_date
            } else {
                // Move to next month
                if today.month() == 12 {
                    chrono::NaiveDate::from_ymd_opt(today.year() + 1, 1, day.min(28))
                } else {
                    chrono::NaiveDate::from_ymd_opt(today.year(), today.month() + 1, day.min(28))
                }.unwrap_or(today)
            }
        } else if let Some(&last_date) = detected.occurrence_dates.last() {
            // Calculate based on frequency from last occurrence
            match detected.frequency {
                RecurrenceFrequency::Weekly => last_date + chrono::Duration::days(7),
                RecurrenceFrequency::Biweekly => last_date + chrono::Duration::days(14),
                RecurrenceFrequency::Monthly => {
                    if last_date.month() == 12 {
                        chrono::NaiveDate::from_ymd_opt(last_date.year() + 1, 1, last_date.day())
                    } else {
                        chrono::NaiveDate::from_ymd_opt(last_date.year(), last_date.month() + 1, last_date.day())
                    }.unwrap_or(last_date + chrono::Duration::days(30))
                },
                RecurrenceFrequency::Quarterly => last_date + chrono::Duration::days(91),
                RecurrenceFrequency::Yearly => {
                    chrono::NaiveDate::from_ymd_opt(last_date.year() + 1, last_date.month(), last_date.day())
                        .unwrap_or(last_date + chrono::Duration::days(365))
                },
                _ => last_date + chrono::Duration::days(30),
            }
        } else {
            today
        };

        // Use first occurrence as start date
        let start_date = detected.occurrence_dates.first().copied().unwrap_or(today);

        let mut recurring = RecurringTransaction::new(
            detected.account_id,
            detected.transaction_type,
            detected.average_amount,
            detected.description,
            detected.frequency,
            start_date,
        );

        recurring.next_occurrence = next_occurrence;
        recurring.category_id = detected.category_id;
        recurring.payee = detected.payee;
        recurring.day_of_month = detected.typical_day_of_month.map(|d| d as i32);
        recurring.reminder_days = Some(3);

        db.with_connection(|conn| {
            repository::create_recurring(conn, &recurring)?;
            Ok(recurring)
        })
    })
}

#[tauri::command]
pub fn deactivate_recurring(
    state: State<AppState>,
    id: String,
    reason: Option<String>,
) -> Result<CancelledSubscription, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| {
            // Get the recurring transaction first
            let recurring = repository::get_recurring_by_id(conn, &id)?;

            // Create a cancelled subscription record
            let cancelled = CancelledSubscription::from_recurring(&recurring, reason);
            repository::create_cancelled_subscription(conn, &cancelled)?;

            // Deactivate the recurring transaction
            repository::deactivate_recurring(conn, &id)?;

            Ok(cancelled)
        })
    })
}

#[tauri::command]
pub fn get_cancelled_subscriptions(state: State<AppState>) -> Result<Vec<CancelledSubscription>, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| repository::get_cancelled_subscriptions(conn))
    })
}

#[tauri::command]
pub fn get_savings_summary(state: State<AppState>) -> Result<SavingsSummary, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| {
            let cancelled = repository::get_cancelled_subscriptions(conn)?;

            let total_yearly: Decimal = cancelled.iter()
                .map(|c| c.estimated_yearly_savings)
                .sum();

            let total_monthly = total_yearly / Decimal::from(12);

            Ok(SavingsSummary {
                total_yearly_savings: total_yearly,
                total_monthly_savings: total_monthly,
                cancelled_count: cancelled.len(),
                cancelled_subscriptions: cancelled,
            })
        })
    })
}

// Goals Commands
#[tauri::command]
pub fn create_goal(state: State<AppState>, request: CreateGoalRequest) -> Result<SavingsGoal, String> {
    with_db(&state, |db| {
        let mut goal = SavingsGoal::new(request.name, request.goal_type, request.target_amount);

        if let Some(current) = request.current_amount {
            goal.current_amount = current;
        }
        goal.target_date = request.target_date;
        goal.account_id = request.account_id;
        if let Some(color) = request.color {
            goal.color = color;
        }
        goal.icon = request.icon;
        goal.notes = request.notes;

        db.with_connection(|conn| {
            repository::create_goal(conn, &goal)?;
            Ok(goal)
        })
    })
}

#[tauri::command]
pub fn get_goals(state: State<AppState>) -> Result<Vec<SavingsGoal>, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| repository::get_all_goals(conn))
    })
}

#[tauri::command]
pub fn update_goal_progress(state: State<AppState>, id: String, amount: Decimal) -> Result<(), String> {
    with_db(&state, |db| {
        db.with_connection(|conn| repository::update_goal_amount(conn, &id, amount))
    })
}

// Import Commands
#[tauri::command]
pub fn detect_import_columns(path: String) -> Result<Vec<String>, String> {
    let path = PathBuf::from(&path);
    let format = importer::detect_file_format(&path).map_err(|e| e.to_string())?;

    match format {
        "csv" => importer::detect_csv_columns(&path).map_err(|e| e.to_string()),
        "excel" => {
            // For Excel, we'd need to read the first row as headers
            // For now, return empty to let preview handle it
            Ok(vec!["Date".to_string(), "Description".to_string(), "Amount".to_string()])
        }
        "ofx" | "qif" => {
            // OFX and QIF have fixed field structures - no column mapping needed
            // Return special marker columns to indicate this to the frontend
            Ok(vec!["__AUTO__".to_string()])
        }
        _ => Err(format!("Unsupported format: {}", format)),
    }
}

#[tauri::command]
pub fn preview_import(path: String, mapping: importer::ImportMapping) -> Result<Vec<importer::RawTransaction>, String> {
    let path = PathBuf::from(&path);
    importer::preview_import(&path, &mapping).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_transactions(
    state: State<AppState>,
    path: String,
    account_id: String,
    mapping: importer::ImportMapping,
) -> Result<importer::ImportResult, String> {
    let path_buf = PathBuf::from(&path);

    // Use import_file which handles all supported formats (CSV, Excel, OFX, QFX, QIF)
    let raw_transactions = importer::import_file(&path_buf, &mapping).map_err(|e| e.to_string())?;

    with_db(&state, |db| {
        db.with_connection(|conn| {
            let rules = repository::get_category_rules(conn)?;
            let categorizer = Categorizer::new(rules);

            let mut imported = Vec::new();
            let mut skipped = 0;
            let mut errors = Vec::new();

            for raw in &raw_transactions {
                match importer::parse_transaction(raw, &account_id, &mapping.date_format) {
                    Ok(mut tx) => {
                        if let Some(ref imported_id) = tx.imported_id {
                            if repository::check_duplicate_transaction(conn, imported_id)? {
                                skipped += 1;
                                continue;
                            }
                        }

                        if tx.category_id.is_none() {
                            tx.category_id = categorizer.categorize(&tx);
                        }

                        repository::create_transaction(conn, &tx)?;
                        imported.push(tx);
                    }
                    Err(e) => {
                        errors.push(format!("Row error: {}", e));
                    }
                }
            }

            Ok(importer::ImportResult {
                imported,
                skipped_duplicates: skipped,
                errors,
            })
        })
    })
}

// Report Commands
#[tauri::command]
pub fn get_spending_by_category(
    state: State<AppState>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> Result<Vec<services::reports::SpendingByCategory>, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| {
            let filter = TransactionFilter {
                account_ids: None,
                category_ids: None,
                transaction_types: Some(vec![TransactionType::Expense]),
                start_date,
                end_date,
                min_amount: None,
                max_amount: None,
                search_text: None,
                status: None,
            };

            let transactions = repository::get_transactions(conn, &filter)?;
            let categories = repository::get_all_categories(conn)?;

            Ok(services::reports::calculate_spending_by_category(&transactions, &categories))
        })
    })
}

#[tauri::command]
pub fn get_monthly_trends(state: State<AppState>, months: Option<usize>) -> Result<Vec<services::reports::MonthlyTrend>, String> {
    let months = months.unwrap_or(12);

    with_db(&state, |db| {
        db.with_connection(|conn| {
            let transactions = repository::get_transactions(conn, &TransactionFilter {
                account_ids: None,
                category_ids: None,
                transaction_types: None,
                start_date: None,
                end_date: None,
                min_amount: None,
                max_amount: None,
                search_text: None,
                status: None,
            })?;

            Ok(services::reports::calculate_monthly_trends(&transactions, months))
        })
    })
}

#[tauri::command]
pub fn get_cash_flow_report(
    state: State<AppState>,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<services::reports::CashFlowReport, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| {
            let transactions = repository::get_transactions(conn, &TransactionFilter {
                account_ids: None,
                category_ids: None,
                transaction_types: None,
                start_date: Some(start_date),
                end_date: Some(end_date),
                min_amount: None,
                max_amount: None,
                search_text: None,
                status: None,
            })?;

            let categories = repository::get_all_categories(conn)?;
            let accounts = repository::get_all_accounts(conn)?;

            let starting_balance: Decimal = accounts.iter().map(|a| a.balance).sum();

            Ok(services::reports::calculate_cash_flow(
                &transactions,
                &categories,
                start_date,
                end_date,
                starting_balance,
            ))
        })
    })
}

// Calendar Commands
#[tauri::command]
pub fn get_calendar_events(
    state: State<AppState>,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<Vec<CalendarEvent>, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| {
            let filter = TransactionFilter {
                account_ids: None,
                category_ids: None,
                transaction_types: None,
                start_date: Some(start_date),
                end_date: Some(end_date),
                min_amount: None,
                max_amount: None,
                search_text: None,
                status: None,
            };

            let transactions = repository::get_transactions(conn, &filter)?;
            let categories = repository::get_all_categories(conn)?;
            let accounts = repository::get_all_accounts(conn)?;

            let category_map: std::collections::HashMap<String, &Category> =
                categories.iter().map(|c| (c.id.clone(), c)).collect();
            let account_map: std::collections::HashMap<String, String> =
                accounts.into_iter().map(|a| (a.id, a.name)).collect();

            let mut events: Vec<CalendarEvent> = transactions
                .iter()
                .map(|tx| {
                    let category = tx.category_id.as_ref().and_then(|id| category_map.get(id));

                    CalendarEvent {
                        id: tx.id.clone(),
                        title: tx.description.clone(),
                        date: tx.date,
                        amount: tx.amount,
                        transaction_type: tx.transaction_type.clone(),
                        category_name: category.map(|c| c.name.clone()),
                        category_color: category.map(|c| c.color.clone()),
                        is_recurring: tx.recurring_id.is_some(),
                        account_name: account_map.get(&tx.account_id).cloned().unwrap_or_default(),
                    }
                })
                .collect();

            let recurring = repository::get_all_recurring(conn)?;
            for r in &recurring {
                let mut next = r.next_occurrence;
                while next <= end_date {
                    if next >= start_date {
                        let category = r.category_id.as_ref().and_then(|id| category_map.get(id));

                        events.push(CalendarEvent {
                            id: format!("{}_{}", r.id, next),
                            title: format!("{} (scheduled)", r.description),
                            date: next,
                            amount: r.amount,
                            transaction_type: r.transaction_type.clone(),
                            category_name: category.map(|c| c.name.clone()),
                            category_color: category.map(|c| c.color.clone()),
                            is_recurring: true,
                            account_name: account_map.get(&r.account_id).cloned().unwrap_or_default(),
                        });
                    }

                    match r.calculate_next_occurrence(next) {
                        Some(n) => next = n,
                        None => break,
                    }
                }
            }

            events.sort_by_key(|e| e.date);
            Ok(events)
        })
    })
}

// Auto-Categorize Command
#[tauri::command]
pub fn auto_categorize_transactions(state: State<AppState>) -> Result<AutoCategorizeResult, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| {
            // Get all rules (user-defined + default)
            let user_rules = repository::get_category_rules(conn)?;

            // Combine with default rules, user rules take priority
            let mut all_rules = user_rules;
            all_rules.extend(services::categorizer::get_default_rules());

            let categorizer = Categorizer::new(all_rules);

            // Get all uncategorized transactions
            let filter = TransactionFilter::default();
            let transactions = repository::get_transactions(conn, &filter)?;

            let mut categorized_count = 0;
            let mut category_breakdown: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

            for tx in &transactions {
                if tx.category_id.is_none() {
                    if let Some(category_id) = categorizer.categorize(tx) {
                        // Update the transaction with the category
                        let mut updated_tx = tx.clone();
                        updated_tx.category_id = Some(category_id.clone());
                        repository::update_transaction(conn, &updated_tx)?;

                        categorized_count += 1;
                        *category_breakdown.entry(category_id).or_insert(0) += 1;
                    }
                }
            }

            // Get category names for the breakdown
            let categories = repository::get_all_categories(conn)?;
            let category_map: std::collections::HashMap<String, String> = categories
                .into_iter()
                .map(|c| (c.id, c.name))
                .collect();

            let breakdown: Vec<CategoryBreakdown> = category_breakdown
                .into_iter()
                .map(|(id, count)| CategoryBreakdown {
                    category_id: id.clone(),
                    category_name: category_map.get(&id).cloned().unwrap_or_default(),
                    count,
                })
                .collect();

            Ok(AutoCategorizeResult {
                total_categorized: categorized_count,
                breakdown,
            })
        })
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutoCategorizeResult {
    pub total_categorized: usize,
    pub breakdown: Vec<CategoryBreakdown>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CategoryBreakdown {
    pub category_id: String,
    pub category_name: String,
    pub count: usize,
}

// Category Rules Commands
#[tauri::command]
pub fn create_category_rule(state: State<AppState>, category_id: String, pattern: String, field: Option<String>) -> Result<CategoryRule, String> {
    with_db(&state, |db| {
        let rule = CategoryRule::new(category_id, pattern, field.unwrap_or_else(|| "description".to_string()));

        db.with_connection(|conn| {
            repository::create_category_rule(conn, &rule)?;
            Ok(rule)
        })
    })
}

#[tauri::command]
pub fn get_category_rules(state: State<AppState>) -> Result<Vec<CategoryRule>, String> {
    with_db(&state, |db| {
        db.with_connection(|conn| repository::get_category_rules(conn))
    })
}

// Notification Commands
#[tauri::command]
pub fn get_bill_reminders(state: State<AppState>, days_ahead: Option<i32>) -> Result<Vec<services::notifications::BillReminder>, String> {
    let days = days_ahead.unwrap_or(7);

    with_db(&state, |db| {
        db.with_connection(|conn| {
            let recurring = repository::get_all_recurring(conn)?;
            let accounts = repository::get_all_accounts(conn)?;
            let categories = repository::get_all_categories(conn)?;

            let account_map: std::collections::HashMap<String, String> =
                accounts.into_iter().map(|a| (a.id, a.name)).collect();
            let category_map: std::collections::HashMap<String, String> =
                categories.into_iter().map(|c| (c.id, c.name)).collect();

            Ok(services::notifications::get_bill_reminders(
                &recurring,
                &account_map,
                &category_map,
                days,
            ))
        })
    })
}

#[tauri::command]
pub fn send_bill_notification(
    app: tauri::AppHandle,
    title: String,
    body: String,
) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;

    app.notification()
        .builder()
        .title(&title)
        .body(&body)
        .show()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn check_and_send_notifications(
    state: State<AppState>,
    app: tauri::AppHandle,
    days_before: i32,
    show_amount: bool,
) -> Result<usize, String> {
    use tauri_plugin_notification::NotificationExt;

    let reminders = with_db(&state, |db| {
        db.with_connection(|conn| {
            let recurring = repository::get_all_recurring(conn)?;
            let accounts = repository::get_all_accounts(conn)?;
            let categories = repository::get_all_categories(conn)?;

            let account_map: std::collections::HashMap<String, String> =
                accounts.into_iter().map(|a| (a.id, a.name)).collect();
            let category_map: std::collections::HashMap<String, String> =
                categories.into_iter().map(|c| (c.id, c.name)).collect();

            Ok(services::notifications::get_bill_reminders(
                &recurring,
                &account_map,
                &category_map,
                days_before,
            ))
        })
    })?;

    let mut sent = 0;
    for reminder in &reminders {
        if reminder.days_until <= days_before as i64 {
            let title = services::notifications::format_notification_title(reminder);
            let body = services::notifications::format_notification_body(reminder, show_amount);

            if app.notification()
                .builder()
                .title(&title)
                .body(&body)
                .show()
                .is_ok()
            {
                sent += 1;
            }
        }
    }

    Ok(sent)
}

// Encryption Commands
#[tauri::command]
pub fn get_encryption_status(state: State<AppState>) -> Result<services::encryption::EncryptionStatus, String> {
    let guard = state.encryption.lock().unwrap();
    let encryption = guard.as_ref().ok_or("Encryption not initialized")?;

    Ok(services::encryption::EncryptionStatus {
        enabled: encryption.is_enabled(),
        unlocked: encryption.is_unlocked(),
    })
}

#[tauri::command]
pub fn enable_encryption(state: State<AppState>, password: String) -> Result<(), String> {
    let mut guard = state.encryption.lock().unwrap();
    let encryption = guard.as_mut().ok_or("Encryption not initialized")?;

    encryption.enable(&password).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn disable_encryption(state: State<AppState>, password: String) -> Result<(), String> {
    let mut guard = state.encryption.lock().unwrap();
    let encryption = guard.as_mut().ok_or("Encryption not initialized")?;

    encryption.disable(&password).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn unlock_encryption(state: State<AppState>, password: String) -> Result<(), String> {
    let mut guard = state.encryption.lock().unwrap();
    let encryption = guard.as_mut().ok_or("Encryption not initialized")?;

    encryption.unlock(&password).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn lock_encryption(state: State<AppState>) -> Result<(), String> {
    let mut guard = state.encryption.lock().unwrap();
    let encryption = guard.as_mut().ok_or("Encryption not initialized")?;

    encryption.lock();
    Ok(())
}

#[tauri::command]
pub fn change_encryption_password(
    state: State<AppState>,
    old_password: String,
    new_password: String,
) -> Result<(), String> {
    let mut guard = state.encryption.lock().unwrap();
    let encryption = guard.as_mut().ok_or("Encryption not initialized")?;

    encryption.change_password(&old_password, &new_password).map_err(|e| e.to_string())
}

// Backup Commands
#[tauri::command]
pub fn export_backup(state: State<AppState>, path: String) -> Result<services::backup::BackupMetadata, String> {
    let path_buf = PathBuf::from(&path);

    with_db(&state, |db| {
        services::backup::export_backup_to_file(db, &path_buf)
            .map_err(|e| crate::database::DatabaseError::Other(e.to_string()))
    })
}

#[tauri::command]
pub fn import_backup(state: State<AppState>, path: String) -> Result<services::backup::BackupMetadata, String> {
    let path_buf = PathBuf::from(&path);

    with_db(&state, |db| {
        services::backup::import_backup_from_file(db, &path_buf)
            .map_err(|e| crate::database::DatabaseError::Other(e.to_string()))
    })
}

#[tauri::command]
pub fn get_backup_info(path: String) -> Result<services::backup::BackupMetadata, String> {
    let path_buf = PathBuf::from(&path);
    services::backup::get_backup_info(&path_buf).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_default_backup_path() -> String {
    services::backup::get_default_backup_path()
        .to_string_lossy()
        .to_string()
}
