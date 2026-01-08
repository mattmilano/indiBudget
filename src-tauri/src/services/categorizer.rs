use regex::Regex;

use crate::models::{CategoryRule, Transaction};

pub struct Categorizer {
    rules: Vec<CompiledRule>,
}

struct CompiledRule {
    category_id: String,
    pattern: String,
    regex: Option<Regex>,
    field: String,
    priority: i32,
}

impl Categorizer {
    pub fn new(rules: Vec<CategoryRule>) -> Self {
        let compiled_rules: Vec<CompiledRule> = rules
            .into_iter()
            .map(|rule| {
                let regex = if rule.is_regex {
                    Regex::new(&rule.pattern).ok()
                } else {
                    None
                };

                CompiledRule {
                    category_id: rule.category_id,
                    pattern: rule.pattern.to_lowercase(),
                    regex,
                    field: rule.field,
                    priority: rule.priority,
                }
            })
            .collect();

        Self { rules: compiled_rules }
    }

    pub fn categorize(&self, transaction: &Transaction) -> Option<String> {
        let mut best_match: Option<(&CompiledRule, i32)> = None;

        for rule in &self.rules {
            let field_value = match rule.field.as_str() {
                "description" => &transaction.description,
                "payee" => transaction.payee.as_ref().unwrap_or(&transaction.description),
                _ => &transaction.description,
            };

            let field_lower = field_value.to_lowercase();

            let matches = if let Some(ref regex) = rule.regex {
                regex.is_match(&field_lower)
            } else {
                field_lower.contains(&rule.pattern)
            };

            if matches {
                if best_match.is_none() || rule.priority > best_match.unwrap().1 {
                    best_match = Some((rule, rule.priority));
                }
            }
        }

        best_match.map(|(rule, _)| rule.category_id.clone())
    }

    pub fn categorize_batch(&self, transactions: &mut [Transaction]) {
        for tx in transactions.iter_mut() {
            if tx.category_id.is_none() {
                tx.category_id = self.categorize(tx);
            }
        }
    }
}

pub fn get_default_rules() -> Vec<CategoryRule> {
    vec![
        CategoryRule::new("cat_expense_groceries".into(), "grocery".into(), "description".into()),
        CategoryRule::new("cat_expense_groceries".into(), "supermarket".into(), "description".into()),
        CategoryRule::new("cat_expense_groceries".into(), "whole foods".into(), "description".into()),
        CategoryRule::new("cat_expense_groceries".into(), "trader joe".into(), "description".into()),
        CategoryRule::new("cat_expense_groceries".into(), "safeway".into(), "description".into()),
        CategoryRule::new("cat_expense_groceries".into(), "kroger".into(), "description".into()),
        CategoryRule::new("cat_expense_groceries".into(), "walmart".into(), "description".into()),
        CategoryRule::new("cat_expense_groceries".into(), "costco".into(), "description".into()),
        CategoryRule::new("cat_expense_dining".into(), "restaurant".into(), "description".into()),
        CategoryRule::new("cat_expense_dining".into(), "doordash".into(), "description".into()),
        CategoryRule::new("cat_expense_dining".into(), "uber eats".into(), "description".into()),
        CategoryRule::new("cat_expense_dining".into(), "grubhub".into(), "description".into()),
        CategoryRule::new("cat_expense_dining".into(), "mcdonald".into(), "description".into()),
        CategoryRule::new("cat_expense_dining".into(), "starbucks".into(), "description".into()),
        CategoryRule::new("cat_expense_dining".into(), "chipotle".into(), "description".into()),
        CategoryRule::new("cat_expense_transportation".into(), "uber".into(), "description".into()),
        CategoryRule::new("cat_expense_transportation".into(), "lyft".into(), "description".into()),
        CategoryRule::new("cat_expense_transportation".into(), "gas station".into(), "description".into()),
        CategoryRule::new("cat_expense_transportation".into(), "shell".into(), "description".into()),
        CategoryRule::new("cat_expense_transportation".into(), "chevron".into(), "description".into()),
        CategoryRule::new("cat_expense_transportation".into(), "exxon".into(), "description".into()),
        CategoryRule::new("cat_expense_utilities".into(), "electric".into(), "description".into()),
        CategoryRule::new("cat_expense_utilities".into(), "water bill".into(), "description".into()),
        CategoryRule::new("cat_expense_utilities".into(), "gas bill".into(), "description".into()),
        CategoryRule::new("cat_expense_utilities".into(), "internet".into(), "description".into()),
        CategoryRule::new("cat_expense_utilities".into(), "comcast".into(), "description".into()),
        CategoryRule::new("cat_expense_utilities".into(), "verizon".into(), "description".into()),
        CategoryRule::new("cat_expense_utilities".into(), "at&t".into(), "description".into()),
        CategoryRule::new("cat_expense_subscriptions".into(), "netflix".into(), "description".into()),
        CategoryRule::new("cat_expense_subscriptions".into(), "spotify".into(), "description".into()),
        CategoryRule::new("cat_expense_subscriptions".into(), "hulu".into(), "description".into()),
        CategoryRule::new("cat_expense_subscriptions".into(), "amazon prime".into(), "description".into()),
        CategoryRule::new("cat_expense_subscriptions".into(), "disney+".into(), "description".into()),
        CategoryRule::new("cat_expense_subscriptions".into(), "apple music".into(), "description".into()),
        CategoryRule::new("cat_expense_shopping".into(), "amazon".into(), "description".into()),
        CategoryRule::new("cat_expense_shopping".into(), "target".into(), "description".into()),
        CategoryRule::new("cat_expense_shopping".into(), "best buy".into(), "description".into()),
        CategoryRule::new("cat_expense_healthcare".into(), "pharmacy".into(), "description".into()),
        CategoryRule::new("cat_expense_healthcare".into(), "cvs".into(), "description".into()),
        CategoryRule::new("cat_expense_healthcare".into(), "walgreens".into(), "description".into()),
        CategoryRule::new("cat_expense_healthcare".into(), "doctor".into(), "description".into()),
        CategoryRule::new("cat_expense_healthcare".into(), "hospital".into(), "description".into()),
        CategoryRule::new("cat_income_salary".into(), "payroll".into(), "description".into()),
        CategoryRule::new("cat_income_salary".into(), "direct deposit".into(), "description".into()),
        CategoryRule::new("cat_income_salary".into(), "salary".into(), "description".into()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TransactionType;
    use chrono::Utc;
    use rust_decimal::Decimal;

    #[test]
    fn test_categorizer_matches_pattern() {
        let rules = vec![
            CategoryRule::new("cat_groceries".into(), "grocery".into(), "description".into()),
        ];

        let categorizer = Categorizer::new(rules);

        let tx = Transaction::new(
            "acc1".into(),
            TransactionType::Expense,
            Decimal::new(5000, 2),
            Utc::now().date_naive(),
            "GROCERY STORE #123".into(),
        );

        assert_eq!(categorizer.categorize(&tx), Some("cat_groceries".into()));
    }
}
