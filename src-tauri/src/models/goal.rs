use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalType {
    Savings,
    DebtPayoff,
    Purchase,
    Emergency,
    Custom,
}

impl GoalType {
    pub fn as_str(&self) -> &'static str {
        match self {
            GoalType::Savings => "savings",
            GoalType::DebtPayoff => "debt_payoff",
            GoalType::Purchase => "purchase",
            GoalType::Emergency => "emergency",
            GoalType::Custom => "custom",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "savings" => GoalType::Savings,
            "debt_payoff" => GoalType::DebtPayoff,
            "purchase" => GoalType::Purchase,
            "emergency" => GoalType::Emergency,
            _ => GoalType::Custom,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Active,
    Completed,
    Paused,
    Cancelled,
}

impl GoalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            GoalStatus::Active => "active",
            GoalStatus::Completed => "completed",
            GoalStatus::Paused => "paused",
            GoalStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "active" => GoalStatus::Active,
            "completed" => GoalStatus::Completed,
            "paused" => GoalStatus::Paused,
            "cancelled" => GoalStatus::Cancelled,
            _ => GoalStatus::Active,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavingsGoal {
    pub id: String,
    pub name: String,
    pub goal_type: GoalType,
    pub target_amount: Decimal,
    pub current_amount: Decimal,
    pub target_date: Option<NaiveDate>,
    pub account_id: Option<String>,
    pub color: String,
    pub icon: Option<String>,
    pub notes: Option<String>,
    pub status: GoalStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SavingsGoal {
    pub fn new(name: String, goal_type: GoalType, target_amount: Decimal) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            goal_type,
            target_amount,
            current_amount: Decimal::ZERO,
            target_date: None,
            account_id: None,
            color: "#3b82f6".to_string(),
            icon: None,
            notes: None,
            status: GoalStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }

    /// Shared by the local Tauri command and the boundary handler.
    pub fn from_request(request: CreateGoalRequest) -> Self {
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
        goal
    }

    pub fn progress_percentage(&self) -> f64 {
        if self.target_amount.is_zero() {
            return 0.0;
        }
        let current: f64 = self.current_amount.try_into().unwrap_or(0.0);
        let target: f64 = self.target_amount.try_into().unwrap_or(1.0);
        (current / target * 100.0).min(100.0)
    }

    pub fn remaining(&self) -> Decimal {
        if self.current_amount >= self.target_amount {
            Decimal::ZERO
        } else {
            self.target_amount - self.current_amount
        }
    }

    pub fn is_completed(&self) -> bool {
        self.current_amount >= self.target_amount
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGoalRequest {
    pub name: String,
    pub goal_type: GoalType,
    pub target_amount: Decimal,
    pub current_amount: Option<Decimal>,
    pub target_date: Option<NaiveDate>,
    pub account_id: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGoalRequest {
    pub id: String,
    pub name: Option<String>,
    pub target_amount: Option<Decimal>,
    pub current_amount: Option<Decimal>,
    pub target_date: Option<NaiveDate>,
    pub account_id: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub notes: Option<String>,
    pub status: Option<GoalStatus>,
}

impl UpdateGoalRequest {
    /// Shared by the local Tauri command and the boundary handler.
    pub fn apply_to(self, goal: &mut SavingsGoal) {
        if let Some(name) = self.name {
            goal.name = name;
        }
        if let Some(target_amount) = self.target_amount {
            goal.target_amount = target_amount;
        }
        if let Some(current_amount) = self.current_amount {
            goal.current_amount = current_amount;
        }
        if let Some(target_date) = self.target_date {
            goal.target_date = Some(target_date);
        }
        if let Some(account_id) = self.account_id {
            goal.account_id = Some(account_id);
        }
        if let Some(color) = self.color {
            goal.color = color;
        }
        if let Some(icon) = self.icon {
            goal.icon = Some(icon);
        }
        if let Some(notes) = self.notes {
            goal.notes = Some(notes);
        }
        if let Some(status) = self.status {
            goal.status = status;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalContribution {
    pub id: String,
    pub goal_id: String,
    pub amount: Decimal,
    pub date: NaiveDate,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl GoalContribution {
    pub fn new(goal_id: String, amount: Decimal, date: NaiveDate) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            goal_id,
            amount,
            date,
            notes: None,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalProgress {
    pub goal: SavingsGoal,
    pub contributions: Vec<GoalContribution>,
    pub monthly_needed: Option<Decimal>,
    pub on_track: bool,
}
