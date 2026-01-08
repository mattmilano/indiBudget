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
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Initialization
            commands::init_app,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
