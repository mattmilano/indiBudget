use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Checking,
    Savings,
    CreditCard,
    Cash,
    Investment,
    Loan,
    Other,
}

impl AccountType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccountType::Checking => "checking",
            AccountType::Savings => "savings",
            AccountType::CreditCard => "credit_card",
            AccountType::Cash => "cash",
            AccountType::Investment => "investment",
            AccountType::Loan => "loan",
            AccountType::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "checking" => AccountType::Checking,
            "savings" => AccountType::Savings,
            "credit_card" => AccountType::CreditCard,
            "cash" => AccountType::Cash,
            "investment" => AccountType::Investment,
            "loan" => AccountType::Loan,
            _ => AccountType::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub account_type: AccountType,
    pub balance: Decimal,
    pub currency: String,
    pub institution: Option<String>,
    pub account_number_last4: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Account {
    pub fn new(name: String, account_type: AccountType) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            account_type,
            balance: Decimal::ZERO,
            currency: "USD".to_string(),
            institution: None,
            account_number_last4: None,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    pub account_type: AccountType,
    pub balance: Option<Decimal>,
    pub currency: Option<String>,
    pub institution: Option<String>,
    pub account_number_last4: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAccountRequest {
    pub id: String,
    pub name: Option<String>,
    pub account_type: Option<AccountType>,
    pub balance: Option<Decimal>,
    pub currency: Option<String>,
    pub institution: Option<String>,
    pub account_number_last4: Option<String>,
    pub is_active: Option<bool>,
}
