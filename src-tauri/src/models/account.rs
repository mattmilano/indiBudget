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
    /// Opening balance when the account was created. The current balance is
    /// derived from this plus all transactions.
    #[serde(default)]
    pub starting_balance: Decimal,
    /// Current balance (computed from starting_balance + transactions).
    /// This is populated by the repository, not stored in the database.
    #[serde(default)]
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
            starting_balance: Decimal::ZERO,
            balance: Decimal::ZERO,
            currency: "USD".to_string(),
            institution: None,
            account_number_last4: None,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// Build an account from a create request.
    ///
    /// Shared by the local Tauri command and the boundary handler, so a remote
    /// caller cannot end up with a different mapping from the one a local
    /// screen uses.
    pub fn from_request(request: CreateAccountRequest) -> Self {
        let mut account = Account::new(request.name, request.account_type);
        if let Some(starting_balance) = request.starting_balance {
            account.starting_balance = starting_balance;
            account.balance = starting_balance;
        }
        if let Some(currency) = request.currency {
            account.currency = currency;
        }
        account.institution = request.institution;
        account.account_number_last4 = request.account_number_last4;
        account
    }

    pub fn with_starting_balance(name: String, account_type: AccountType, starting_balance: Decimal) -> Self {
        let mut account = Self::new(name, account_type);
        account.starting_balance = starting_balance;
        account.balance = starting_balance;
        account
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    pub account_type: AccountType,
    /// Opening balance for the account (optional, defaults to 0)
    #[serde(alias = "balance")]
    pub starting_balance: Option<Decimal>,
    pub currency: Option<String>,
    pub institution: Option<String>,
    pub account_number_last4: Option<String>,
}

impl UpdateAccountRequest {
    /// Merge this request into an existing account.
    ///
    /// Shared by the local Tauri command and the boundary handler, so a remote
    /// write cannot apply a different merge from the one a local screen does.
    pub fn apply_to(self, account: &mut Account) {
        if let Some(name) = self.name {
            account.name = name;
        }
        if let Some(account_type) = self.account_type {
            account.account_type = account_type;
        }
        if let Some(starting_balance) = self.starting_balance {
            account.starting_balance = starting_balance;
        }
        if let Some(currency) = self.currency {
            account.currency = currency;
        }
        if let Some(institution) = self.institution {
            account.institution = Some(institution);
        }
        if let Some(last4) = self.account_number_last4 {
            account.account_number_last4 = Some(last4);
        }
        if let Some(is_active) = self.is_active {
            account.is_active = is_active;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAccountRequest {
    pub id: String,
    pub name: Option<String>,
    pub account_type: Option<AccountType>,
    /// Starting balance can only be adjusted, not the current computed balance
    #[serde(alias = "balance")]
    pub starting_balance: Option<Decimal>,
    pub currency: Option<String>,
    pub institution: Option<String>,
    pub account_number_last4: Option<String>,
    pub is_active: Option<bool>,
}
