use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::models::{RecurringTransaction, TransactionType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillReminder {
    pub recurring_id: String,
    pub description: String,
    pub amount: String,
    pub due_date: NaiveDate,
    pub days_until: i64,
    pub transaction_type: TransactionType,
    pub account_name: String,
    pub category_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub enabled: bool,
    pub days_before: i32,
    pub show_amount: bool,
    pub sound: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            days_before: 3,
            show_amount: true,
            sound: true,
        }
    }
}

pub fn get_bill_reminders(
    recurring: &[RecurringTransaction],
    accounts: &std::collections::HashMap<String, String>,
    categories: &std::collections::HashMap<String, String>,
    days_ahead: i32,
) -> Vec<BillReminder> {
    let today = Utc::now().date_naive();
    let end_date = today + chrono::Duration::days(days_ahead as i64);

    let mut reminders: Vec<BillReminder> = recurring
        .iter()
        .filter(|r| {
            r.is_active
                && r.next_occurrence >= today
                && r.next_occurrence <= end_date
                && r.reminder_days.is_some()
        })
        .map(|r| {
            let days_until = (r.next_occurrence - today).num_days();
            let account_name = accounts.get(&r.account_id).cloned().unwrap_or_default();
            let category_name = r
                .category_id
                .as_ref()
                .and_then(|id| categories.get(id).cloned());

            BillReminder {
                recurring_id: r.id.clone(),
                description: r.description.clone(),
                amount: r.amount.to_string(),
                due_date: r.next_occurrence,
                days_until,
                transaction_type: r.transaction_type.clone(),
                account_name,
                category_name,
            }
        })
        .collect();

    reminders.sort_by_key(|r| r.due_date);
    reminders
}

pub fn format_notification_title(reminder: &BillReminder) -> String {
    if reminder.days_until == 0 {
        format!("{} Due Today", reminder.description)
    } else if reminder.days_until == 1 {
        format!("{} Due Tomorrow", reminder.description)
    } else {
        format!(
            "{} Due in {} Days",
            reminder.description, reminder.days_until
        )
    }
}

pub fn format_notification_body(reminder: &BillReminder, show_amount: bool) -> String {
    let type_str = match reminder.transaction_type {
        TransactionType::Expense => "Payment",
        TransactionType::Income => "Income",
        TransactionType::Transfer => "Transfer",
    };

    if show_amount {
        format!(
            "{}: ${} - {}",
            type_str,
            reminder.amount,
            reminder.due_date.format("%B %d, %Y")
        )
    } else {
        format!(
            "{} due on {}",
            type_str,
            reminder.due_date.format("%B %d, %Y")
        )
    }
}
