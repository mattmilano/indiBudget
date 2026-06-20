//! Tier 2 — Auto-categorization (footgun F8).
//!
//! F8: user rules outrank system rules; first-match-by-priority is
//!     deterministic; batch categorization is idempotent and never overwrites a
//!     manual category; regex and literal rules both work.

mod common;

use common::*;
use indibudget_lib::models::*;
use indibudget_lib::services::categorizer::Categorizer;

fn tx(description: &str) -> Transaction {
    Transaction::new(
        "acct".to_string(),
        TransactionType::Expense,
        dec("10.00"),
        date("2026-06-01"),
        description.to_string(),
    )
}

fn literal_rule(category_id: &str, pattern: &str, priority: i32) -> CategoryRule {
    CategoryRule::with_priority(
        category_id.to_string(),
        pattern.to_string(),
        "description".to_string(),
        priority,
    )
}

#[test]
fn user_rule_outranks_system_rule_for_same_match() {
    // Same pattern, two rules: user (priority 100) must win over system (0).
    let system = literal_rule("sys_cat", "coffee", 0);
    let user = literal_rule("user_cat", "coffee", 100);

    let cz = Categorizer::new(vec![system, user]);
    assert_eq!(
        cz.categorize(&tx("Morning Coffee Run")).as_deref(),
        Some("user_cat"),
        "higher-priority (user) rule must win"
    );
}

#[test]
fn literal_match_is_case_insensitive() {
    let cz = Categorizer::new(vec![literal_rule("groceries", "walmart", 10)]);
    assert_eq!(
        cz.categorize(&tx("WALMART #4021")).as_deref(),
        Some("groceries")
    );
}

#[test]
fn regex_rule_matches() {
    let mut rule = literal_rule("shopping", r"^amz\b.*", 10);
    rule.is_regex = true;
    let cz = Categorizer::new(vec![rule]);
    assert_eq!(
        cz.categorize(&tx("AMZ Marketplace Order")).as_deref(),
        Some("shopping"),
        "regex rule must match"
    );
    assert_eq!(
        cz.categorize(&tx("Local Bakery")).as_deref(),
        None,
        "non-matching description must stay uncategorized"
    );
}

#[test]
fn categorization_is_deterministic_and_idempotent() {
    let cz = Categorizer::new(vec![
        literal_rule("a", "store", 0),
        literal_rule("b", "store", 50),
    ]);
    let first = cz.categorize(&tx("Corner Store")).clone();
    let second = cz.categorize(&tx("Corner Store")).clone();
    assert_eq!(first, second, "same input must always yield same category");
    assert_eq!(first.as_deref(), Some("b"), "deterministic highest-priority winner");
}

#[test]
fn batch_never_overwrites_a_manual_category() {
    // categorize_batch must only fill in uncategorized rows.
    let cz = Categorizer::new(vec![literal_rule("auto_cat", "coffee", 10)]);

    let mut manual = tx("Coffee Shop");
    manual.category_id = Some("manual_choice".to_string());
    let mut auto = tx("Coffee Shop");

    let mut batch = vec![manual, auto.clone()];
    cz.categorize_batch(&mut batch);

    assert_eq!(
        batch[0].category_id.as_deref(),
        Some("manual_choice"),
        "a manually-set category must never be clobbered"
    );
    assert_eq!(
        batch[1].category_id.as_deref(),
        Some("auto_cat"),
        "an uncategorized row must be auto-categorized"
    );

    // Re-running the batch is idempotent.
    cz.categorize_batch(&mut batch);
    assert_eq!(batch[0].category_id.as_deref(), Some("manual_choice"));
    assert_eq!(batch[1].category_id.as_deref(), Some("auto_cat"));

    let _ = &mut auto; // silence unused-mut on the clone source
}
