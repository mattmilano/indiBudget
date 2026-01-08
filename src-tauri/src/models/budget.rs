use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPeriod {
    Weekly,
    Biweekly,
    Monthly,
    Quarterly,
    Yearly,
}

impl BudgetPeriod {
    pub fn as_str(&self) -> &'static str {
        match self {
            BudgetPeriod::Weekly => "weekly",
            BudgetPeriod::Biweekly => "biweekly",
            BudgetPeriod::Monthly => "monthly",
            BudgetPeriod::Quarterly => "quarterly",
            BudgetPeriod::Yearly => "yearly",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "weekly" => BudgetPeriod::Weekly,
            "biweekly" => BudgetPeriod::Biweekly,
            "monthly" => BudgetPeriod::Monthly,
            "quarterly" => BudgetPeriod::Quarterly,
            "yearly" => BudgetPeriod::Yearly,
            _ => BudgetPeriod::Monthly,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub id: String,
    pub name: String,
    pub category_id: String,
    pub amount: Decimal,
    pub period: BudgetPeriod,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub rollover: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Budget {
    pub fn new(
        name: String,
        category_id: String,
        amount: Decimal,
        period: BudgetPeriod,
        start_date: NaiveDate,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            category_id,
            amount,
            period,
            start_date,
            end_date: None,
            rollover: false,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBudgetRequest {
    pub name: String,
    pub category_id: String,
    pub amount: Decimal,
    pub period: BudgetPeriod,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub rollover: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBudgetRequest {
    pub id: String,
    pub name: Option<String>,
    pub amount: Option<Decimal>,
    pub period: Option<BudgetPeriod>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub rollover: Option<bool>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub budget: Budget,
    pub category_name: String,
    pub category_color: String,
    pub spent: Decimal,
    pub remaining: Decimal,
    pub percentage_used: f64,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub is_over_budget: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSummary {
    pub total_budgeted: Decimal,
    pub total_spent: Decimal,
    pub total_remaining: Decimal,
    pub budgets: Vec<BudgetStatus>,
}
