use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::models::{Budget, BudgetPeriod, BudgetStatus, Category, Transaction, TransactionType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingByCategory {
    pub category_id: String,
    pub category_name: String,
    pub category_color: String,
    pub total: Decimal,
    pub percentage: f64,
    pub transaction_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyTrend {
    pub month: String,
    pub year: i32,
    pub income: Decimal,
    pub expenses: Decimal,
    pub net: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyBalance {
    pub date: NaiveDate,
    pub balance: Decimal,
    pub income: Decimal,
    pub expenses: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashFlowReport {
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub total_income: Decimal,
    pub total_expenses: Decimal,
    pub net_cash_flow: Decimal,
    pub income_by_category: Vec<SpendingByCategory>,
    pub expenses_by_category: Vec<SpendingByCategory>,
    pub daily_balances: Vec<DailyBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeVsExpenses {
    pub period: String,
    pub income: Decimal,
    pub expenses: Decimal,
    pub savings_rate: f64,
}

pub fn calculate_spending_by_category(
    transactions: &[Transaction],
    categories: &[Category],
) -> Vec<SpendingByCategory> {
    let category_map: HashMap<String, &Category> =
        categories.iter().map(|c| (c.id.clone(), c)).collect();

    let mut spending: HashMap<String, (Decimal, usize)> = HashMap::new();
    let mut total = Decimal::ZERO;

    for tx in transactions {
        if tx.transaction_type == TransactionType::Expense {
            let cat_id = tx
                .category_id
                .clone()
                .unwrap_or_else(|| "uncategorized".to_string());
            let entry = spending.entry(cat_id).or_insert((Decimal::ZERO, 0));
            entry.0 += tx.amount;
            entry.1 += 1;
            total += tx.amount;
        }
    }

    let mut result: Vec<SpendingByCategory> = spending
        .into_iter()
        .map(|(cat_id, (amount, count))| {
            let (name, color) = if cat_id == "uncategorized" {
                ("Uncategorized".to_string(), "#6b7280".to_string())
            } else {
                category_map
                    .get(&cat_id)
                    .map(|c| (c.name.clone(), c.color.clone()))
                    .unwrap_or_else(|| ("Unknown".to_string(), "#6b7280".to_string()))
            };

            let percentage = if total.is_zero() {
                0.0
            } else {
                let amt: f64 = amount.try_into().unwrap_or(0.0);
                let tot: f64 = total.try_into().unwrap_or(1.0);
                (amt / tot) * 100.0
            };

            SpendingByCategory {
                category_id: cat_id,
                category_name: name,
                category_color: color,
                total: amount,
                percentage,
                transaction_count: count,
            }
        })
        .collect();

    result.sort_by(|a, b| b.total.cmp(&a.total));
    result
}

pub fn calculate_monthly_trends(transactions: &[Transaction], months: usize) -> Vec<MonthlyTrend> {
    let mut monthly: HashMap<(i32, u32), (Decimal, Decimal)> = HashMap::new();

    for tx in transactions {
        let key = (tx.date.year(), tx.date.month());
        let entry = monthly.entry(key).or_insert((Decimal::ZERO, Decimal::ZERO));

        match tx.transaction_type {
            TransactionType::Income => entry.0 += tx.amount,
            TransactionType::Expense => entry.1 += tx.amount,
            TransactionType::Transfer => {}
        }
    }

    let mut result: Vec<MonthlyTrend> = monthly
        .into_iter()
        .map(|((year, month), (income, expenses))| {
            let month_name = match month {
                1 => "Jan",
                2 => "Feb",
                3 => "Mar",
                4 => "Apr",
                5 => "May",
                6 => "Jun",
                7 => "Jul",
                8 => "Aug",
                9 => "Sep",
                10 => "Oct",
                11 => "Nov",
                12 => "Dec",
                _ => "Unknown",
            };

            MonthlyTrend {
                month: month_name.to_string(),
                year,
                income,
                expenses,
                net: income - expenses,
            }
        })
        .collect();

    result.sort_by(|a, b| {
        let a_key = (a.year, month_to_num(&a.month));
        let b_key = (b.year, month_to_num(&b.month));
        a_key.cmp(&b_key)
    });

    if result.len() > months {
        result = result.into_iter().rev().take(months).collect();
        result.reverse();
    }

    result
}

fn month_to_num(month: &str) -> u32 {
    match month {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => 0,
    }
}

pub fn calculate_cash_flow(
    transactions: &[Transaction],
    categories: &[Category],
    start_date: NaiveDate,
    end_date: NaiveDate,
    starting_balance: Decimal,
) -> CashFlowReport {
    let filtered: Vec<&Transaction> = transactions
        .iter()
        .filter(|tx| tx.date >= start_date && tx.date <= end_date)
        .collect();

    let mut total_income = Decimal::ZERO;
    let mut total_expenses = Decimal::ZERO;

    let income_txs: Vec<Transaction> = filtered
        .iter()
        .filter(|tx| tx.transaction_type == TransactionType::Income)
        .map(|tx| {
            total_income += tx.amount;
            (*tx).clone()
        })
        .collect();

    let expense_txs: Vec<Transaction> = filtered
        .iter()
        .filter(|tx| tx.transaction_type == TransactionType::Expense)
        .map(|tx| {
            total_expenses += tx.amount;
            (*tx).clone()
        })
        .collect();

    let income_by_category = calculate_spending_by_category(&income_txs, categories);
    let expenses_by_category = calculate_spending_by_category(&expense_txs, categories);

    let mut daily_map: HashMap<NaiveDate, (Decimal, Decimal)> = HashMap::new();
    for tx in &filtered {
        let entry = daily_map
            .entry(tx.date)
            .or_insert((Decimal::ZERO, Decimal::ZERO));
        match tx.transaction_type {
            TransactionType::Income => entry.0 += tx.amount,
            TransactionType::Expense => entry.1 += tx.amount,
            TransactionType::Transfer => {}
        }
    }

    let mut daily_balances: Vec<DailyBalance> = Vec::new();
    let mut running_balance = starting_balance;
    let mut current = start_date;

    while current <= end_date {
        let (income, expenses) = daily_map
            .get(&current)
            .copied()
            .unwrap_or((Decimal::ZERO, Decimal::ZERO));
        running_balance = running_balance + income - expenses;

        daily_balances.push(DailyBalance {
            date: current,
            balance: running_balance,
            income,
            expenses,
        });

        current = current.succ_opt().unwrap_or(current);
    }

    CashFlowReport {
        period_start: start_date,
        period_end: end_date,
        total_income,
        total_expenses,
        net_cash_flow: total_income - total_expenses,
        income_by_category,
        expenses_by_category,
        daily_balances,
    }
}

pub fn calculate_budget_status(
    budget: &Budget,
    transactions: &[Transaction],
    categories: &[Category],
    as_of_date: NaiveDate,
) -> BudgetStatus {
    let (period_start, period_end) = get_budget_period_dates(budget, as_of_date);

    let spent: Decimal = transactions
        .iter()
        .filter(|tx| {
            tx.transaction_type == TransactionType::Expense
                && tx.date >= period_start
                && tx.date <= period_end
                && tx.category_id.as_ref() == Some(&budget.category_id)
        })
        .map(|tx| tx.amount)
        .sum();

    let remaining = budget.amount - spent;
    let percentage_used = if budget.amount.is_zero() {
        0.0
    } else {
        let s: f64 = spent.try_into().unwrap_or(0.0);
        let b: f64 = budget.amount.try_into().unwrap_or(1.0);
        (s / b) * 100.0
    };

    let category = categories.iter().find(|c| c.id == budget.category_id);

    BudgetStatus {
        budget: budget.clone(),
        category_name: category
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Unknown".to_string()),
        category_color: category
            .map(|c| c.color.clone())
            .unwrap_or_else(|| "#6b7280".to_string()),
        spent,
        remaining,
        percentage_used,
        period_start,
        period_end,
        is_over_budget: spent > budget.amount,
    }
}

fn get_budget_period_dates(budget: &Budget, as_of_date: NaiveDate) -> (NaiveDate, NaiveDate) {
    match budget.period {
        BudgetPeriod::Weekly => {
            let weekday = as_of_date.weekday().num_days_from_monday();
            let start = as_of_date - chrono::Duration::days(weekday as i64);
            let end = start + chrono::Duration::days(6);
            (start, end)
        }
        BudgetPeriod::Biweekly => {
            let days_since_start = (as_of_date - budget.start_date).num_days();
            let period_num = days_since_start / 14;
            let start = budget.start_date + chrono::Duration::days(period_num * 14);
            let end = start + chrono::Duration::days(13);
            (start, end)
        }
        BudgetPeriod::Monthly => {
            // First day of current month - always valid for valid as_of_date
            let start = NaiveDate::from_ymd_opt(as_of_date.year(), as_of_date.month(), 1)
                .unwrap_or(as_of_date);
            // Last day of current month: go to first of next month, subtract 1 day
            let end = if as_of_date.month() == 12 {
                NaiveDate::from_ymd_opt(as_of_date.year() + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(as_of_date.year(), as_of_date.month() + 1, 1)
            }
            .map(|d| d - chrono::Duration::days(1))
            .unwrap_or(as_of_date);
            (start, end)
        }
        BudgetPeriod::Quarterly => {
            let quarter = (as_of_date.month() - 1) / 3;
            let start_month = quarter * 3 + 1;
            let start =
                NaiveDate::from_ymd_opt(as_of_date.year(), start_month, 1).unwrap_or(as_of_date);
            let end_month = start_month + 2;
            let end = if end_month == 12 {
                NaiveDate::from_ymd_opt(as_of_date.year() + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(as_of_date.year(), end_month + 1, 1)
            }
            .map(|d| d - chrono::Duration::days(1))
            .unwrap_or(as_of_date);
            (start, end)
        }
        BudgetPeriod::Yearly => {
            let start = NaiveDate::from_ymd_opt(as_of_date.year(), 1, 1).unwrap_or(as_of_date);
            let end = NaiveDate::from_ymd_opt(as_of_date.year(), 12, 31).unwrap_or(as_of_date);
            (start, end)
        }
    }
}

pub fn calculate_projected_balance(
    current_balance: Decimal,
    transactions: &[Transaction],
    target_date: NaiveDate,
) -> Decimal {
    let today = chrono::Utc::now().date_naive();

    let future_transactions: Decimal = transactions
        .iter()
        .filter(|tx| tx.date > today && tx.date <= target_date)
        .map(|tx| match tx.transaction_type {
            TransactionType::Income => tx.amount,
            TransactionType::Expense => -tx.amount,
            TransactionType::Transfer => Decimal::ZERO,
        })
        .sum();

    current_balance + future_transactions
}
