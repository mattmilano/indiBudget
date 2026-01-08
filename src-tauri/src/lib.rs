pub mod commands;
pub mod database;
pub mod models;
pub mod services;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Initialization
            commands::init_app,
            commands::get_database_path,
            commands::get_transaction_count,
            // Accounts
            commands::create_account,
            commands::get_accounts,
            commands::get_account,
            commands::update_account,
            commands::delete_account,
            // Transactions
            commands::create_transaction,
            commands::get_transactions,
            commands::get_transaction,
            commands::update_transaction,
            commands::delete_transaction,
            // Categories
            commands::get_categories,
            commands::create_category,
            // Budgets
            commands::create_budget,
            commands::get_budgets,
            commands::get_budget_status,
            // Recurring
            commands::create_recurring,
            commands::get_recurring,
            commands::get_upcoming_recurring,
            commands::detect_recurring_patterns,
            commands::create_recurring_from_detected,
            // Goals
            commands::create_goal,
            commands::get_goals,
            commands::update_goal_progress,
            // Import
            commands::detect_import_columns,
            commands::preview_import,
            commands::import_transactions,
            // Reports
            commands::get_spending_by_category,
            commands::get_monthly_trends,
            commands::get_cash_flow_report,
            // Calendar
            commands::get_calendar_events,
            // Category Rules
            commands::create_category_rule,
            commands::get_category_rules,
            // Notifications
            commands::get_bill_reminders,
            commands::send_bill_notification,
            commands::check_and_send_notifications,
            // Encryption
            commands::get_encryption_status,
            commands::enable_encryption,
            commands::disable_encryption,
            commands::unlock_encryption,
            commands::lock_encryption,
            commands::change_encryption_password,
            // Backup
            commands::export_backup,
            commands::import_backup,
            commands::get_backup_info,
            commands::get_default_backup_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
