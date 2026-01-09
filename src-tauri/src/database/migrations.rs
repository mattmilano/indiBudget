use rusqlite::Connection;

use super::DbResult;

pub fn run_all(conn: &Connection) -> DbResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY
        );",
    )?;

    let current_version: i32 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    let migrations: Vec<(&str, i32)> = vec![
        (MIGRATION_001_INITIAL, 1),
        (MIGRATION_002_RECURRING, 2),
        (MIGRATION_003_BUDGETS, 3),
        (MIGRATION_004_GOALS, 4),
        (MIGRATION_005_IMPORT_RULES, 5),
        (MIGRATION_006_CANCELLED_SUBSCRIPTIONS, 6),
        (MIGRATION_007_USER_CATEGORY_RULES, 7),
    ];

    for (sql, version) in migrations {
        if version > current_version {
            conn.execute_batch(sql)?;
            conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [version])?;
        }
    }

    Ok(())
}

const MIGRATION_001_INITIAL: &str = r#"
    CREATE TABLE IF NOT EXISTS accounts (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        account_type TEXT NOT NULL,
        balance TEXT NOT NULL DEFAULT '0',
        currency TEXT NOT NULL DEFAULT 'USD',
        institution TEXT,
        account_number_last4 TEXT,
        is_active INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS categories (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        category_type TEXT NOT NULL,
        color TEXT NOT NULL DEFAULT '#6b7280',
        icon TEXT,
        parent_id TEXT REFERENCES categories(id),
        is_system INTEGER NOT NULL DEFAULT 0,
        is_active INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS transactions (
        id TEXT PRIMARY KEY,
        account_id TEXT NOT NULL REFERENCES accounts(id),
        transaction_type TEXT NOT NULL,
        amount TEXT NOT NULL,
        date TEXT NOT NULL,
        description TEXT NOT NULL,
        category_id TEXT REFERENCES categories(id),
        payee TEXT,
        notes TEXT,
        status TEXT NOT NULL DEFAULT 'cleared',
        is_split INTEGER NOT NULL DEFAULT 0,
        parent_transaction_id TEXT REFERENCES transactions(id),
        recurring_id TEXT,
        transfer_account_id TEXT REFERENCES accounts(id),
        imported_id TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_transactions_account ON transactions(account_id);
    CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date);
    CREATE INDEX IF NOT EXISTS idx_transactions_category ON transactions(category_id);
    CREATE INDEX IF NOT EXISTS idx_transactions_imported_id ON transactions(imported_id);
"#;

const MIGRATION_002_RECURRING: &str = r#"
    CREATE TABLE IF NOT EXISTS recurring_transactions (
        id TEXT PRIMARY KEY,
        account_id TEXT NOT NULL REFERENCES accounts(id),
        transaction_type TEXT NOT NULL,
        amount TEXT NOT NULL,
        description TEXT NOT NULL,
        category_id TEXT REFERENCES categories(id),
        payee TEXT,
        frequency TEXT NOT NULL,
        start_date TEXT NOT NULL,
        end_date TEXT,
        next_occurrence TEXT NOT NULL,
        day_of_month INTEGER,
        day_of_week INTEGER,
        auto_post INTEGER NOT NULL DEFAULT 0,
        reminder_days INTEGER DEFAULT 3,
        is_active INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_recurring_next ON recurring_transactions(next_occurrence);
    CREATE INDEX IF NOT EXISTS idx_recurring_account ON recurring_transactions(account_id);
"#;

const MIGRATION_003_BUDGETS: &str = r#"
    CREATE TABLE IF NOT EXISTS budgets (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        category_id TEXT NOT NULL REFERENCES categories(id),
        amount TEXT NOT NULL,
        period TEXT NOT NULL,
        start_date TEXT NOT NULL,
        end_date TEXT,
        rollover INTEGER NOT NULL DEFAULT 0,
        is_active INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_budgets_category ON budgets(category_id);
"#;

const MIGRATION_004_GOALS: &str = r#"
    CREATE TABLE IF NOT EXISTS savings_goals (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        goal_type TEXT NOT NULL,
        target_amount TEXT NOT NULL,
        current_amount TEXT NOT NULL DEFAULT '0',
        target_date TEXT,
        account_id TEXT REFERENCES accounts(id),
        color TEXT NOT NULL DEFAULT '#3b82f6',
        icon TEXT,
        notes TEXT,
        status TEXT NOT NULL DEFAULT 'active',
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS goal_contributions (
        id TEXT PRIMARY KEY,
        goal_id TEXT NOT NULL REFERENCES savings_goals(id),
        amount TEXT NOT NULL,
        date TEXT NOT NULL,
        notes TEXT,
        created_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_goal_contributions_goal ON goal_contributions(goal_id);
"#;

const MIGRATION_005_IMPORT_RULES: &str = r#"
    CREATE TABLE IF NOT EXISTS category_rules (
        id TEXT PRIMARY KEY,
        category_id TEXT NOT NULL REFERENCES categories(id),
        pattern TEXT NOT NULL,
        field TEXT NOT NULL DEFAULT 'description',
        is_regex INTEGER NOT NULL DEFAULT 0,
        priority INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS import_history (
        id TEXT PRIMARY KEY,
        filename TEXT NOT NULL,
        account_id TEXT NOT NULL REFERENCES accounts(id),
        imported_count INTEGER NOT NULL,
        duplicate_count INTEGER NOT NULL,
        import_date TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_category_rules_category ON category_rules(category_id);
"#;

const MIGRATION_006_CANCELLED_SUBSCRIPTIONS: &str = r#"
    CREATE TABLE IF NOT EXISTS cancelled_subscriptions (
        id TEXT PRIMARY KEY,
        recurring_id TEXT NOT NULL,
        description TEXT NOT NULL,
        amount TEXT NOT NULL,
        frequency TEXT NOT NULL,
        cancelled_at TEXT NOT NULL,
        reason TEXT,
        estimated_yearly_savings TEXT NOT NULL,
        created_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_cancelled_subscriptions_date ON cancelled_subscriptions(cancelled_at);
"#;

const MIGRATION_007_USER_CATEGORY_RULES: &str = r#"
    -- Add is_user_created column to distinguish user rules from system defaults
    ALTER TABLE category_rules ADD COLUMN is_user_created INTEGER NOT NULL DEFAULT 0;

    -- User rules get highest priority (100) by default to override system rules
    CREATE INDEX IF NOT EXISTS idx_category_rules_user ON category_rules(is_user_created);
"#;
