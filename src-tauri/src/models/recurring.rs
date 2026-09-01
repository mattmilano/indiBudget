use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::TransactionType;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecurrenceFrequency {
    Daily,
    Weekly,
    Biweekly,
    Monthly,
    Quarterly,
    Yearly,
}

impl RecurrenceFrequency {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecurrenceFrequency::Daily => "daily",
            RecurrenceFrequency::Weekly => "weekly",
            RecurrenceFrequency::Biweekly => "biweekly",
            RecurrenceFrequency::Monthly => "monthly",
            RecurrenceFrequency::Quarterly => "quarterly",
            RecurrenceFrequency::Yearly => "yearly",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "daily" => RecurrenceFrequency::Daily,
            "weekly" => RecurrenceFrequency::Weekly,
            "biweekly" => RecurrenceFrequency::Biweekly,
            "monthly" => RecurrenceFrequency::Monthly,
            "quarterly" => RecurrenceFrequency::Quarterly,
            "yearly" => RecurrenceFrequency::Yearly,
            _ => RecurrenceFrequency::Monthly,
        }
    }

    pub fn days_between(&self) -> i64 {
        match self {
            RecurrenceFrequency::Daily => 1,
            RecurrenceFrequency::Weekly => 7,
            RecurrenceFrequency::Biweekly => 14,
            RecurrenceFrequency::Monthly => 30,
            RecurrenceFrequency::Quarterly => 91,
            RecurrenceFrequency::Yearly => 365,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringTransaction {
    pub id: String,
    pub account_id: String,
    pub transaction_type: TransactionType,
    pub amount: Decimal,
    pub description: String,
    pub category_id: Option<String>,
    pub payee: Option<String>,
    pub frequency: RecurrenceFrequency,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub next_occurrence: NaiveDate,
    pub day_of_month: Option<i32>,
    pub day_of_week: Option<i32>,
    pub auto_post: bool,
    pub reminder_days: Option<i32>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl RecurringTransaction {
    pub fn new(
        account_id: String,
        transaction_type: TransactionType,
        amount: Decimal,
        description: String,
        frequency: RecurrenceFrequency,
        start_date: NaiveDate,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            account_id,
            transaction_type,
            amount,
            description,
            category_id: None,
            payee: None,
            frequency,
            start_date,
            end_date: None,
            next_occurrence: start_date,
            day_of_month: None,
            day_of_week: None,
            auto_post: false,
            reminder_days: Some(3),
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// Shared by the local Tauri command and the boundary handler.
    pub fn from_request(request: CreateRecurringRequest) -> Self {
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
        recurring
    }

    pub fn calculate_next_occurrence(&self, from_date: NaiveDate) -> Option<NaiveDate> {
        use chrono::Datelike;

        if let Some(end) = self.end_date {
            if from_date >= end {
                return None;
            }
        }

        let next = match self.frequency {
            RecurrenceFrequency::Daily => from_date.succ_opt(),
            RecurrenceFrequency::Weekly => from_date.checked_add_days(chrono::Days::new(7)),
            RecurrenceFrequency::Biweekly => from_date.checked_add_days(chrono::Days::new(14)),
            RecurrenceFrequency::Monthly => {
                let day = self.day_of_month.unwrap_or(from_date.day() as i32);
                let next_month = if from_date.month() == 12 {
                    NaiveDate::from_ymd_opt(from_date.year() + 1, 1, day as u32)
                } else {
                    NaiveDate::from_ymd_opt(from_date.year(), from_date.month() + 1, day as u32)
                };
                next_month.or_else(|| {
                    let last_day = if from_date.month() == 12 {
                        NaiveDate::from_ymd_opt(from_date.year() + 1, 2, 1)
                    } else {
                        NaiveDate::from_ymd_opt(from_date.year(), from_date.month() + 2, 1)
                    };
                    last_day.and_then(|d| d.pred_opt())
                })
            }
            RecurrenceFrequency::Quarterly => from_date.checked_add_months(chrono::Months::new(3)),
            RecurrenceFrequency::Yearly => from_date.checked_add_months(chrono::Months::new(12)),
        };

        if let (Some(next_date), Some(end)) = (next, self.end_date) {
            if next_date > end {
                return None;
            }
        }

        next
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRecurringRequest {
    pub account_id: String,
    pub transaction_type: TransactionType,
    pub amount: Decimal,
    pub description: String,
    pub category_id: Option<String>,
    pub payee: Option<String>,
    pub frequency: RecurrenceFrequency,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub day_of_month: Option<i32>,
    pub auto_post: Option<bool>,
    pub reminder_days: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRecurringRequest {
    pub id: String,
    pub amount: Option<Decimal>,
    pub description: Option<String>,
    pub category_id: Option<String>,
    pub payee: Option<String>,
    pub frequency: Option<RecurrenceFrequency>,
    pub end_date: Option<NaiveDate>,
    pub day_of_month: Option<i32>,
    pub auto_post: Option<bool>,
    pub reminder_days: Option<i32>,
    pub is_active: Option<bool>,
}

impl UpdateRecurringRequest {
    /// Shared by the local Tauri command and the boundary handler.
    pub fn apply_to(self, recurring: &mut RecurringTransaction) {
        if let Some(amount) = self.amount {
            recurring.amount = amount;
        }
        if let Some(description) = self.description {
            recurring.description = description;
        }
        if let Some(category_id) = self.category_id {
            recurring.category_id = Some(category_id);
        }
        if let Some(payee) = self.payee {
            recurring.payee = Some(payee);
        }
        if let Some(frequency) = self.frequency {
            recurring.frequency = frequency;
        }
        if let Some(end_date) = self.end_date {
            recurring.end_date = Some(end_date);
        }
        if let Some(day_of_month) = self.day_of_month {
            recurring.day_of_month = Some(day_of_month);
        }
        if let Some(auto_post) = self.auto_post {
            recurring.auto_post = auto_post;
        }
        if let Some(reminder_days) = self.reminder_days {
            recurring.reminder_days = Some(reminder_days);
        }
        if let Some(is_active) = self.is_active {
            recurring.is_active = is_active;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpcomingRecurring {
    pub recurring: RecurringTransaction,
    pub next_date: NaiveDate,
    pub days_until: i64,
    pub category_name: Option<String>,
    pub account_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelledSubscription {
    pub id: String,
    pub recurring_id: String,
    pub description: String,
    pub amount: Decimal,
    pub frequency: RecurrenceFrequency,
    pub cancelled_at: NaiveDate,
    pub reason: Option<String>,
    pub estimated_yearly_savings: Decimal,
    pub created_at: DateTime<Utc>,
}

impl CancelledSubscription {
    pub fn from_recurring(recurring: &RecurringTransaction, reason: Option<String>) -> Self {
        let yearly_savings = Self::calculate_yearly_savings(&recurring.frequency, recurring.amount);
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            recurring_id: recurring.id.clone(),
            description: recurring.description.clone(),
            amount: recurring.amount,
            frequency: recurring.frequency.clone(),
            cancelled_at: now.date_naive(),
            reason,
            estimated_yearly_savings: yearly_savings,
            created_at: now,
        }
    }

    pub fn calculate_yearly_savings(frequency: &RecurrenceFrequency, amount: Decimal) -> Decimal {
        let multiplier = match frequency {
            RecurrenceFrequency::Daily => Decimal::from(365),
            RecurrenceFrequency::Weekly => Decimal::from(52),
            RecurrenceFrequency::Biweekly => Decimal::from(26),
            RecurrenceFrequency::Monthly => Decimal::from(12),
            RecurrenceFrequency::Quarterly => Decimal::from(4),
            RecurrenceFrequency::Yearly => Decimal::from(1),
        };
        amount.abs() * multiplier
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavingsSummary {
    pub total_yearly_savings: Decimal,
    pub total_monthly_savings: Decimal,
    pub cancelled_count: usize,
    pub cancelled_subscriptions: Vec<CancelledSubscription>,
}
