use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::models::{CategoryRule, RecurrenceFrequency, Transaction, TransactionType};

/// A detected recurring transaction pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedRecurring {
    /// Normalized description used for grouping
    pub description: String,
    /// Original payee if available
    pub payee: Option<String>,
    /// The detected frequency
    pub frequency: RecurrenceFrequency,
    /// Average amount (positive for expenses, negative for income)
    pub average_amount: Decimal,
    /// Whether the amount is consistent across occurrences
    pub amount_is_consistent: bool,
    /// The transaction type
    pub transaction_type: TransactionType,
    /// Number of occurrences found
    pub occurrence_count: usize,
    /// Dates of occurrences
    pub occurrence_dates: Vec<NaiveDate>,
    /// Typical day of month (for monthly transactions)
    pub typical_day_of_month: Option<u32>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Account ID from the transactions
    pub account_id: String,
    /// Category ID if consistently categorized
    pub category_id: Option<String>,
    /// Suggested category ID from categorization rules (used when no consistent category exists)
    pub suggested_category_id: Option<String>,
}

/// Analyze transactions and detect recurring patterns
pub fn detect_recurring_transactions(transactions: &[Transaction]) -> Vec<DetectedRecurring> {
    // Group transactions by normalized description
    let mut groups: HashMap<String, Vec<&Transaction>> = HashMap::new();

    for tx in transactions {
        let key = normalize_description(&tx.description);
        groups.entry(key).or_default().push(tx);
    }

    let mut detected: Vec<DetectedRecurring> = Vec::new();

    for (description, txs) in groups {
        // Need at least 2 occurrences to detect a pattern
        if txs.len() < 2 {
            continue;
        }

        // Sort by date
        let mut sorted_txs: Vec<&Transaction> = txs.clone();
        sorted_txs.sort_by_key(|t| t.date);

        // Analyze the pattern
        if let Some(pattern) = analyze_pattern(&description, &sorted_txs) {
            detected.push(pattern);
        }
    }

    // Sort by confidence (highest first)
    detected.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

    detected
}

/// Normalize a description for grouping similar transactions
fn normalize_description(desc: &str) -> String {
    let normalized = desc.to_lowercase();

    // Remove common suffixes/prefixes that vary
    let normalized = normalized
        .trim()
        // Remove trailing numbers (like transaction IDs)
        .trim_end_matches(|c: char| c.is_numeric() || c == '#')
        .trim()
        // Remove common date patterns
        .split_whitespace()
        .filter(|word| {
            // Filter out date-like words
            !word.chars().all(|c| c.is_numeric() || c == '/' || c == '-')
        })
        .collect::<Vec<_>>()
        .join(" ");

    normalized
}

/// Analyze a group of transactions to detect a recurring pattern
fn analyze_pattern(description: &str, transactions: &[&Transaction]) -> Option<DetectedRecurring> {
    if transactions.len() < 2 {
        return None;
    }

    // Calculate intervals between consecutive transactions
    let mut intervals: Vec<i64> = Vec::new();
    for i in 1..transactions.len() {
        let days = (transactions[i].date - transactions[i-1].date).num_days();
        intervals.push(days);
    }

    // Detect frequency based on average interval
    let avg_interval: f64 = intervals.iter().sum::<i64>() as f64 / intervals.len() as f64;
    let (frequency, expected_interval) = detect_frequency(avg_interval);

    // Calculate interval consistency (how close intervals are to expected)
    let interval_variance: f64 = intervals.iter()
        .map(|&i| (i as f64 - expected_interval).powi(2))
        .sum::<f64>() / intervals.len() as f64;
    let interval_stddev = interval_variance.sqrt();

    // If intervals are too inconsistent, skip
    // Allow more variance for longer intervals
    let max_stddev = match frequency {
        RecurrenceFrequency::Weekly => 3.0,
        RecurrenceFrequency::Biweekly => 5.0,
        RecurrenceFrequency::Monthly => 7.0,
        RecurrenceFrequency::Quarterly => 15.0,
        RecurrenceFrequency::Yearly => 30.0,
        _ => 5.0,
    };

    if interval_stddev > max_stddev {
        return None;
    }

    // Analyze amounts
    let amounts: Vec<Decimal> = transactions.iter().map(|t| t.amount).collect();
    let avg_amount = amounts.iter().copied().sum::<Decimal>() / Decimal::from(amounts.len());

    // Check amount consistency (within 10% of average)
    let amount_is_consistent = amounts.iter().all(|&a| {
        let diff = (a - avg_amount).abs();
        let threshold = avg_amount.abs() * Decimal::new(10, 2); // 10%
        diff <= threshold
    });

    // Get typical day of month for monthly transactions
    let typical_day_of_month = if matches!(frequency, RecurrenceFrequency::Monthly) {
        let days: Vec<u32> = transactions.iter().map(|t| t.date.day()).collect();
        let avg_day = days.iter().sum::<u32>() / days.len() as u32;
        Some(avg_day)
    } else {
        None
    };

    // Calculate confidence score
    let confidence = calculate_confidence(
        transactions.len(),
        interval_stddev,
        expected_interval,
        amount_is_consistent,
    );

    // Only return if confidence is reasonable
    if confidence < 0.5 {
        return None;
    }

    // Get the most common transaction type
    let transaction_type = transactions.first().map(|t| t.transaction_type.clone())
        .unwrap_or(TransactionType::Expense);

    // Get account ID (use the most recent transaction's account)
    let account_id = transactions.last().map(|t| t.account_id.clone())
        .unwrap_or_default();

    // Get category if consistent
    let category_id = {
        let categories: Vec<_> = transactions.iter()
            .filter_map(|t| t.category_id.as_ref())
            .collect();
        if !categories.is_empty() && categories.iter().all(|&c| c == categories[0]) {
            Some(categories[0].clone())
        } else {
            None
        }
    };

    // Get payee from first transaction that has one
    let payee = transactions.iter()
        .find_map(|t| t.payee.clone());

    Some(DetectedRecurring {
        description: description.to_string(),
        payee,
        frequency,
        average_amount: avg_amount,
        amount_is_consistent,
        transaction_type,
        occurrence_count: transactions.len(),
        occurrence_dates: transactions.iter().map(|t| t.date).collect(),
        typical_day_of_month,
        confidence,
        account_id,
        category_id,
        suggested_category_id: None, // Will be populated by enhance_with_category_suggestions
    })
}

/// Detect frequency based on average interval in days
fn detect_frequency(avg_interval: f64) -> (RecurrenceFrequency, f64) {
    // Define expected intervals for each frequency
    let frequencies = [
        (RecurrenceFrequency::Weekly, 7.0),
        (RecurrenceFrequency::Biweekly, 14.0),
        (RecurrenceFrequency::Monthly, 30.0),
        (RecurrenceFrequency::Quarterly, 91.0),
        (RecurrenceFrequency::Yearly, 365.0),
    ];

    // Find the closest match
    frequencies.iter()
        .min_by(|(_, a), (_, b)| {
            let diff_a = (avg_interval - a).abs();
            let diff_b = (avg_interval - b).abs();
            diff_a.partial_cmp(&diff_b).unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
        .unwrap_or((RecurrenceFrequency::Monthly, 30.0))
}

/// Calculate confidence score based on various factors
fn calculate_confidence(
    occurrence_count: usize,
    interval_stddev: f64,
    expected_interval: f64,
    amount_is_consistent: bool,
) -> f64 {
    let mut confidence = 0.0;

    // More occurrences = higher confidence (up to 0.4)
    let occurrence_factor = (occurrence_count as f64 / 12.0).min(1.0) * 0.4;
    confidence += occurrence_factor;

    // Lower interval variance = higher confidence (up to 0.3)
    let relative_stddev = interval_stddev / expected_interval;
    let interval_factor = (1.0 - relative_stddev.min(1.0)) * 0.3;
    confidence += interval_factor;

    // Consistent amount adds confidence (0.2)
    if amount_is_consistent {
        confidence += 0.2;
    }

    // Base confidence for having a pattern (0.1)
    confidence += 0.1;

    confidence
}

/// Enhance detected recurring patterns with category suggestions from rules
/// This uses the categorization rules to suggest categories for patterns
/// that don't have a consistent category from their transactions
pub fn enhance_with_category_suggestions(
    detected: &mut Vec<DetectedRecurring>,
    rules: &[CategoryRule],
) {
    use crate::services::categorizer::Categorizer;

    let categorizer = Categorizer::new(rules.to_vec());

    for pattern in detected.iter_mut() {
        // Skip if already has a consistent category
        if pattern.category_id.is_some() {
            continue;
        }

        // Try to match using the description or payee
        // Create a temporary transaction-like structure for matching
        let description = pattern.payee.as_ref().unwrap_or(&pattern.description);

        // Try matching against description patterns
        if let Some(category_id) = categorizer.categorize_text(description) {
            pattern.suggested_category_id = Some(category_id);
        } else if let Some(category_id) = categorizer.categorize_text(&pattern.description) {
            pattern.suggested_category_id = Some(category_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_description() {
        assert_eq!(normalize_description("NETFLIX.COM 123456"), "netflix.com");
        assert_eq!(normalize_description("Spotify USA"), "spotify usa");
    }

    #[test]
    fn test_detect_frequency() {
        assert!(matches!(detect_frequency(7.0).0, RecurrenceFrequency::Weekly));
        assert!(matches!(detect_frequency(14.0).0, RecurrenceFrequency::Biweekly));
        assert!(matches!(detect_frequency(30.0).0, RecurrenceFrequency::Monthly));
        assert!(matches!(detect_frequency(28.0).0, RecurrenceFrequency::Monthly));
    }
}
