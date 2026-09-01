pub mod boundary;
pub mod commands;
pub mod database;
pub mod models;
pub mod net;
pub mod services;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Initialization
            // Multi-user: hosting and connecting. Local-only by design — none
            // of these are registered in the boundary registry.
            commands::multiuser::hosting_status,
            commands::multiuser::start_hosting,
            commands::multiuser::stop_hosting,
            commands::multiuser::open_pairing,
            commands::multiuser::close_pairing,
            commands::multiuser::pair_with_host,
            commands::multiuser::connect_to_host,
            commands::multiuser::disconnect_from_host,
            commands::multiuser::boundary_invoke,
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
            commands::create_transfer,
            // Categories
            commands::get_categories,
            commands::get_category,
            commands::create_category,
            commands::update_category,
            commands::delete_category,
            // Budgets
            commands::create_budget,
            commands::get_budgets,
            commands::get_budget,
            commands::update_budget,
            commands::delete_budget,
            commands::get_budget_status,
            // Recurring
            commands::create_recurring,
            commands::get_recurring,
            commands::get_recurring_by_id,
            commands::update_recurring,
            commands::get_upcoming_recurring,
            commands::detect_recurring_patterns,
            commands::create_recurring_from_detected,
            commands::deactivate_recurring,
            commands::get_cancelled_subscriptions,
            commands::get_savings_summary,
            // Goals
            commands::create_goal,
            commands::get_goals,
            commands::get_goal,
            commands::update_goal,
            commands::delete_goal,
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
            // Category Rules & Auto-Categorize
            commands::create_category_rule,
            commands::get_category_rules,
            commands::get_user_category_rules,
            commands::delete_user_category_rule,
            commands::auto_categorize_transactions,
            commands::batch_categorize_transactions,
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
            // App Settings (secure storage)
            commands::get_setting,
            commands::set_setting,
            commands::delete_setting,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
