use std::collections::HashMap;
use std::path::Path;

use calamine::{open_workbook, Reader, Xlsx};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

use crate::models::{Transaction, TransactionStatus, TransactionType};

#[derive(Error, Debug)]
pub enum ImportError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
    #[error("Excel error: {0}")]
    Excel(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportMapping {
    pub date_column: String,
    pub description_column: String,
    pub amount_column: String,
    pub debit_column: Option<String>,
    pub credit_column: Option<String>,
    pub category_column: Option<String>,
    pub date_format: String,
    pub has_header: bool,
    pub skip_rows: usize,
}

impl Default for ImportMapping {
    fn default() -> Self {
        Self {
            date_column: "Date".to_string(),
            description_column: "Description".to_string(),
            amount_column: "Amount".to_string(),
            debit_column: None,
            credit_column: None,
            category_column: None,
            date_format: "%m/%d/%Y".to_string(),
            has_header: true,
            skip_rows: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: Vec<Transaction>,
    pub skipped_duplicates: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTransaction {
    pub date: String,
    pub description: String,
    pub amount: String,
    pub debit: Option<String>,
    pub credit: Option<String>,
    pub category: Option<String>,
}

pub fn detect_file_format(path: &Path) -> Result<&'static str, ImportError> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .ok_or_else(|| ImportError::UnsupportedFormat("No file extension".to_string()))?;

    match extension.as_str() {
        "csv" => Ok("csv"),
        "xlsx" | "xls" => Ok("excel"),
        "ofx" | "qfx" => Ok("ofx"),
        "qif" => Ok("qif"),
        _ => Err(ImportError::UnsupportedFormat(extension)),
    }
}

pub fn import_csv(path: &Path, mapping: &ImportMapping) -> Result<Vec<RawTransaction>, ImportError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(mapping.has_header)
        .from_path(path)?;

    let headers: HashMap<String, usize> = if mapping.has_header {
        reader
            .headers()?
            .iter()
            .enumerate()
            .map(|(i, h)| (h.to_lowercase(), i))
            .collect()
    } else {
        HashMap::new()
    };

    let date_idx = headers
        .get(&mapping.date_column.to_lowercase())
        .copied()
        .unwrap_or(0);
    let desc_idx = headers
        .get(&mapping.description_column.to_lowercase())
        .copied()
        .unwrap_or(1);
    let amount_idx = headers
        .get(&mapping.amount_column.to_lowercase())
        .copied()
        .unwrap_or(2);
    let debit_idx = mapping
        .debit_column
        .as_ref()
        .and_then(|c| headers.get(&c.to_lowercase()).copied());
    let credit_idx = mapping
        .credit_column
        .as_ref()
        .and_then(|c| headers.get(&c.to_lowercase()).copied());

    let mut transactions = Vec::new();

    for (i, result) in reader.records().enumerate() {
        if i < mapping.skip_rows {
            continue;
        }

        let record = result?;

        let date = record.get(date_idx).unwrap_or("").to_string();
        let description = record.get(desc_idx).unwrap_or("").to_string();

        let amount = if let (Some(debit), Some(credit)) = (debit_idx, credit_idx) {
            let debit_val = record.get(debit).unwrap_or("").trim();
            let credit_val = record.get(credit).unwrap_or("").trim();

            if !debit_val.is_empty() && debit_val != "0" && debit_val != "0.00" {
                format!("-{}", debit_val.replace(['$', ',', ' '], ""))
            } else if !credit_val.is_empty() && credit_val != "0" && credit_val != "0.00" {
                credit_val.replace(['$', ',', ' '], "")
            } else {
                "0".to_string()
            }
        } else {
            record
                .get(amount_idx)
                .unwrap_or("0")
                .replace(['$', ',', ' '], "")
        };

        if date.is_empty() || description.is_empty() {
            continue;
        }

        transactions.push(RawTransaction {
            date,
            description,
            amount,
            debit: debit_idx.and_then(|i| record.get(i).map(String::from)),
            credit: credit_idx.and_then(|i| record.get(i).map(String::from)),
            category: None,
        });
    }

    Ok(transactions)
}

pub fn import_excel(path: &Path, mapping: &ImportMapping) -> Result<Vec<RawTransaction>, ImportError> {
    let mut workbook: Xlsx<_> =
        open_workbook(path).map_err(|e| ImportError::Excel(e.to_string()))?;

    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| ImportError::Excel("No sheets found".to_string()))?;

    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|e| ImportError::Excel(e.to_string()))?;

    let mut transactions = Vec::new();
    let mut headers: HashMap<String, usize> = HashMap::new();

    for (row_idx, row) in range.rows().enumerate() {
        if row_idx < mapping.skip_rows {
            continue;
        }

        if mapping.has_header && row_idx == mapping.skip_rows {
            for (col_idx, cell) in row.iter().enumerate() {
                if let Some(val) = cell.get_string() {
                    headers.insert(val.to_lowercase(), col_idx);
                }
            }
            continue;
        }

        let date_idx = headers
            .get(&mapping.date_column.to_lowercase())
            .copied()
            .unwrap_or(0);
        let desc_idx = headers
            .get(&mapping.description_column.to_lowercase())
            .copied()
            .unwrap_or(1);
        let amount_idx = headers
            .get(&mapping.amount_column.to_lowercase())
            .copied()
            .unwrap_or(2);

        let date = row
            .get(date_idx)
            .map(|c| c.to_string())
            .unwrap_or_default();
        let description = row
            .get(desc_idx)
            .map(|c| c.to_string())
            .unwrap_or_default();
        let amount = row
            .get(amount_idx)
            .map(|c| c.to_string().replace(['$', ',', ' '], ""))
            .unwrap_or_else(|| "0".to_string());

        if date.is_empty() || description.is_empty() {
            continue;
        }

        transactions.push(RawTransaction {
            date,
            description,
            amount,
            debit: None,
            credit: None,
            category: None,
        });
    }

    Ok(transactions)
}

pub fn parse_transaction(
    raw: &RawTransaction,
    account_id: &str,
    date_format: &str,
) -> Result<Transaction, ImportError> {
    let date = parse_date(&raw.date, date_format)?;

    let amount_str = raw.amount.replace(['$', ',', ' ', '+'], "");
    let amount = Decimal::from_str(&amount_str)
        .map_err(|_| ImportError::Parse(format!("Invalid amount: {}", raw.amount)))?;

    let transaction_type = if amount < Decimal::ZERO {
        TransactionType::Expense
    } else {
        TransactionType::Income
    };

    let imported_id = generate_import_id(&raw.date, &raw.description, &raw.amount);

    let mut tx = Transaction::new(
        account_id.to_string(),
        transaction_type,
        amount.abs(),
        date,
        raw.description.clone(),
    );

    tx.imported_id = Some(imported_id);
    tx.status = TransactionStatus::Cleared;

    Ok(tx)
}

fn parse_date(date_str: &str, format: &str) -> Result<NaiveDate, ImportError> {
    let formats = vec![
        format,
        "%Y-%m-%d",
        "%m/%d/%Y",
        "%m/%d/%y",
        "%d/%m/%Y",
        "%Y/%m/%d",
        "%m-%d-%Y",
        "%d-%m-%Y",
    ];

    for fmt in formats {
        if let Ok(date) = NaiveDate::parse_from_str(date_str.trim(), fmt) {
            return Ok(date);
        }
    }

    let date_str = date_str.trim();
    if let Ok(timestamp) = date_str.parse::<f64>() {
        let days = timestamp as i64 - 25569;
        if let Some(date) = NaiveDate::from_ymd_opt(1970, 1, 1)
            .and_then(|d| d.checked_add_days(chrono::Days::new(days as u64)))
        {
            return Ok(date);
        }
    }

    Err(ImportError::Parse(format!(
        "Could not parse date: {}",
        date_str
    )))
}

fn generate_import_id(date: &str, description: &str, amount: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    date.hash(&mut hasher);
    description.hash(&mut hasher);
    amount.hash(&mut hasher);

    format!("{:x}", hasher.finish())
}

pub fn preview_import(path: &Path, mapping: &ImportMapping) -> Result<Vec<RawTransaction>, ImportError> {
    let format = detect_file_format(path)?;

    let raw_transactions = match format {
        "csv" => import_csv(path, mapping)?,
        "excel" => import_excel(path, mapping)?,
        _ => return Err(ImportError::UnsupportedFormat(format.to_string())),
    };

    Ok(raw_transactions.into_iter().take(10).collect())
}

pub fn detect_csv_columns(path: &Path) -> Result<Vec<String>, ImportError> {
    let mut reader = csv::ReaderBuilder::new().has_headers(true).from_path(path)?;

    let headers: Vec<String> = reader.headers()?.iter().map(String::from).collect();

    Ok(headers)
}
