use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Income,
    Expense,
    Transfer,
}

impl TransactionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionType::Income => "income",
            TransactionType::Expense => "expense",
            TransactionType::Transfer => "transfer",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "income" => TransactionType::Income,
            "expense" => TransactionType::Expense,
            "transfer" => TransactionType::Transfer,
            _ => TransactionType::Expense,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Pending,
    Cleared,
    Reconciled,
    Void,
}

impl TransactionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionStatus::Pending => "pending",
            TransactionStatus::Cleared => "cleared",
            TransactionStatus::Reconciled => "reconciled",
            TransactionStatus::Void => "void",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => TransactionStatus::Pending,
            "cleared" => TransactionStatus::Cleared,
            "reconciled" => TransactionStatus::Reconciled,
            "void" => TransactionStatus::Void,
            _ => TransactionStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub account_id: String,
    pub transaction_type: TransactionType,
    pub amount: Decimal,
    pub date: NaiveDate,
    pub description: String,
    pub category_id: Option<String>,
    pub payee: Option<String>,
    pub notes: Option<String>,
    pub status: TransactionStatus,
    pub is_split: bool,
    pub parent_transaction_id: Option<String>,
    pub recurring_id: Option<String>,
    pub transfer_account_id: Option<String>,
    /// Links the two sides of a transfer together. Both transactions share
    /// the same transfer_pair_id so deleting/editing one can update the other.
    pub transfer_pair_id: Option<String>,
    pub imported_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Transaction {
    pub fn new(
        account_id: String,
        transaction_type: TransactionType,
        amount: Decimal,
        date: NaiveDate,
        description: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            account_id,
            transaction_type,
            amount,
            date,
            description,
            category_id: None,
            payee: None,
            notes: None,
            status: TransactionStatus::Cleared,
            is_split: false,
            parent_transaction_id: None,
            recurring_id: None,
            transfer_account_id: None,
            transfer_pair_id: None,
            imported_id: None,
            created_at: now,
            updated_at: now,
        }
    }
}

impl Transaction {
    /// Shared by the local Tauri command and the boundary handler.
    pub fn from_request(request: CreateTransactionRequest) -> Self {
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
        tx.parent_transaction_id = request.parent_transaction_id;
        tx
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTransactionRequest {
    pub account_id: String,
    pub transaction_type: TransactionType,
    pub amount: Decimal,
    pub date: NaiveDate,
    pub description: String,
    pub category_id: Option<String>,
    pub payee: Option<String>,
    pub notes: Option<String>,
    pub status: Option<TransactionStatus>,
    pub transfer_account_id: Option<String>,
    pub parent_transaction_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTransactionRequest {
    pub id: String,
    pub account_id: Option<String>,
    pub transaction_type: Option<TransactionType>,
    pub amount: Option<Decimal>,
    pub date: Option<NaiveDate>,
    pub description: Option<String>,
    pub category_id: Option<String>,
    pub payee: Option<String>,
    pub notes: Option<String>,
    pub status: Option<TransactionStatus>,
    pub is_split: Option<bool>,
}

impl UpdateTransactionRequest {
    /// Shared by the local Tauri command and the boundary handler.
    pub fn apply_to(self, tx: &mut Transaction) {
        if let Some(account_id) = self.account_id {
            tx.account_id = account_id;
        }
        if let Some(transaction_type) = self.transaction_type {
            tx.transaction_type = transaction_type;
        }
        if let Some(amount) = self.amount {
            tx.amount = amount;
        }
        if let Some(date) = self.date {
            tx.date = date;
        }
        if let Some(description) = self.description {
            tx.description = description;
        }
        if let Some(category_id) = self.category_id {
            tx.category_id = Some(category_id);
        }
        if let Some(payee) = self.payee {
            tx.payee = Some(payee);
        }
        if let Some(notes) = self.notes {
            tx.notes = Some(notes);
        }
        if let Some(status) = self.status {
            tx.status = status;
        }
        if let Some(is_split) = self.is_split {
            tx.is_split = is_split;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitTransaction {
    pub amount: Decimal,
    pub category_id: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransactionFilter {
    pub account_ids: Option<Vec<String>>,
    pub category_ids: Option<Vec<String>>,
    pub transaction_types: Option<Vec<TransactionType>>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub min_amount: Option<Decimal>,
    pub max_amount: Option<Decimal>,
    pub search_text: Option<String>,
    pub status: Option<Vec<TransactionStatus>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub date: NaiveDate,
    pub amount: Decimal,
    pub transaction_type: TransactionType,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub is_recurring: bool,
    pub account_name: String,
}

/// Request to create a transfer between two accounts.
/// Creates two linked transactions (expense from source, income to destination).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTransferRequest {
    pub from_account_id: String,
    pub to_account_id: String,
    pub amount: Decimal,
    pub date: NaiveDate,
    pub description: Option<String>,
    pub notes: Option<String>,
}

/// Response from creating a transfer, returns both transaction IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
    pub from_transaction_id: String,
    pub to_transaction_id: String,
    pub transfer_pair_id: String,
}
