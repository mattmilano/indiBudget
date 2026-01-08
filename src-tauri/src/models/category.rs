use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CategoryType {
    Income,
    Expense,
}

impl CategoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CategoryType::Income => "income",
            CategoryType::Expense => "expense",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "income" => CategoryType::Income,
            _ => CategoryType::Expense,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub category_type: CategoryType,
    pub color: String,
    pub icon: Option<String>,
    pub parent_id: Option<String>,
    pub is_system: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Category {
    pub fn new(name: String, category_type: CategoryType, color: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            category_type,
            color,
            icon: None,
            parent_id: None,
            is_system: false,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn system(id: &str, name: &str, category_type: CategoryType, color: &str) -> Self {
        let now = Utc::now();
        Self {
            id: id.to_string(),
            name: name.to_string(),
            category_type,
            color: color.to_string(),
            icon: None,
            parent_id: None,
            is_system: true,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCategoryRequest {
    pub name: String,
    pub category_type: CategoryType,
    pub color: String,
    pub icon: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCategoryRequest {
    pub id: String,
    pub name: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub parent_id: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryRule {
    pub id: String,
    pub category_id: String,
    pub pattern: String,
    pub field: String,
    pub is_regex: bool,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
}

impl CategoryRule {
    pub fn new(category_id: String, pattern: String, field: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            category_id,
            pattern,
            field,
            is_regex: false,
            priority: 0,
            created_at: Utc::now(),
        }
    }
}

pub fn get_default_categories() -> Vec<Category> {
    vec![
        // Income categories
        Category::system("cat_income_salary", "Salary", CategoryType::Income, "#22c55e"),
        Category::system("cat_income_freelance", "Freelance", CategoryType::Income, "#10b981"),
        Category::system("cat_income_investment", "Investment Income", CategoryType::Income, "#14b8a6"),
        Category::system("cat_income_other", "Other Income", CategoryType::Income, "#06b6d4"),

        // Housing & Utilities
        Category::system("cat_expense_housing", "Housing/Rent", CategoryType::Expense, "#ef4444"),
        Category::system("cat_expense_mortgage", "Mortgage", CategoryType::Expense, "#dc2626"),
        Category::system("cat_expense_utilities", "Utilities", CategoryType::Expense, "#f97316"),

        // Food
        Category::system("cat_expense_groceries", "Groceries", CategoryType::Expense, "#eab308"),
        Category::system("cat_expense_dining", "Dining Out", CategoryType::Expense, "#06b6d4"),
        Category::system("cat_expense_coffee", "Coffee & Cafes", CategoryType::Expense, "#92400e"),

        // Transportation
        Category::system("cat_expense_transportation", "Transportation", CategoryType::Expense, "#84cc16"),
        Category::system("cat_expense_gas", "Gas & Fuel", CategoryType::Expense, "#65a30d"),
        Category::system("cat_expense_rideshare", "Rideshare", CategoryType::Expense, "#4d7c0f"),

        // Health & Insurance
        Category::system("cat_expense_healthcare", "Healthcare", CategoryType::Expense, "#22c55e"),
        Category::system("cat_expense_insurance", "Insurance", CategoryType::Expense, "#14b8a6"),

        // Shopping & Entertainment
        Category::system("cat_expense_shopping", "Shopping", CategoryType::Expense, "#8b5cf6"),
        Category::system("cat_expense_entertainment", "Entertainment", CategoryType::Expense, "#3b82f6"),

        // Subscriptions - broken down by type
        Category::system("cat_expense_subscriptions", "Subscriptions", CategoryType::Expense, "#ec4899"),
        Category::system("cat_expense_streaming", "Streaming Services", CategoryType::Expense, "#db2777"),
        Category::system("cat_expense_news", "News & Magazines", CategoryType::Expense, "#be185d"),
        Category::system("cat_expense_apps", "Apps & Software", CategoryType::Expense, "#9d174d"),
        Category::system("cat_expense_music", "Music Services", CategoryType::Expense, "#831843"),
        Category::system("cat_expense_gaming", "Gaming Subscriptions", CategoryType::Expense, "#701a75"),

        // Other
        Category::system("cat_expense_personal", "Personal Care", CategoryType::Expense, "#a855f7"),
        Category::system("cat_expense_education", "Education", CategoryType::Expense, "#d946ef"),
        Category::system("cat_expense_fees", "Fees & Charges", CategoryType::Expense, "#f43f5e"),
        Category::system("cat_expense_travel", "Travel", CategoryType::Expense, "#0ea5e9"),
        Category::system("cat_expense_pets", "Pets", CategoryType::Expense, "#fb923c"),
        Category::system("cat_expense_gifts", "Gifts & Donations", CategoryType::Expense, "#f472b6"),
        Category::system("cat_expense_other", "Other Expenses", CategoryType::Expense, "#6b7280"),
    ]
}
