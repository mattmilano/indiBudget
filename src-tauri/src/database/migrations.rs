use rusqlite::Connection;

use super::DbResult;

pub fn run_all(conn: &Connection) -> DbResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY
        );",
    )?;

    let current_version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let migrations: Vec<(&str, i32)> = vec![
        (MIGRATION_001_INITIAL, 1),
        (MIGRATION_002_RECURRING, 2),
        (MIGRATION_003_BUDGETS, 3),
        (MIGRATION_004_GOALS, 4),
        (MIGRATION_005_IMPORT_RULES, 5),
        (MIGRATION_006_CANCELLED_SUBSCRIPTIONS, 6),
        (MIGRATION_007_USER_CATEGORY_RULES, 7),
        (MIGRATION_008_DERIVED_BALANCES, 8),
        (MIGRATION_009_APP_SETTINGS, 9),
        (MIGRATION_010_MULTI_USER_BOUNDARY, 10),
        (MIGRATION_011_USERS_AND_GRANTS, 11),
        (MIGRATION_012_PAIRED_DEVICES, 12),
    ];

    for (sql, version) in migrations {
        if version > current_version {
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [version],
            )?;
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

const MIGRATION_008_DERIVED_BALANCES: &str = r#"
    -- Rename balance to starting_balance: this is the account's opening balance,
    -- and the current balance is now derived from transactions.
    ALTER TABLE accounts RENAME COLUMN balance TO starting_balance;

    -- Add transfer_pair_id to link the two sides of a transfer together.
    -- Both transactions in a transfer share the same transfer_pair_id.
    ALTER TABLE transactions ADD COLUMN transfer_pair_id TEXT;

    -- Index for efficient transfer pair lookups
    CREATE INDEX IF NOT EXISTS idx_transactions_transfer_pair ON transactions(transfer_pair_id);

    -- Index to speed up balance computation (account + type for SUM queries)
    CREATE INDEX IF NOT EXISTS idx_transactions_balance ON transactions(account_id, transaction_type);
"#;

const MIGRATION_009_APP_SETTINGS: &str = r#"
    -- App settings table for storing sensitive configuration
    -- (SimpleFIN credentials, preferences, etc.) in the database
    -- instead of localStorage for better security
    CREATE TABLE IF NOT EXISTS app_settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
"#;

// Multi-user phase 1: the boundary.
//
// `row_version` is maintained by an AFTER UPDATE trigger per table. This gets
// the optimistic-concurrency backstop into every user-editable table without
// touching a single repository UPDATE, and it cannot be forgotten by new code.
// SQLite's `recursive_triggers` is OFF by default and this codebase never
// enables it, so a trigger updating its own table cannot re-fire.
//
// `created_by` / `updated_by` are deliberately NOT maintained by triggers. A
// trigger would need per-session actor state on the connection, and there are
// legitimate raw connections (tests, backup/restore, future CLI or salvage
// paths) that would break on a trigger referencing state they do not carry.
// These columns are stamped explicitly by the boundary write wrappers instead,
// and stay NULL for rows written outside the boundary.
const MIGRATION_010_MULTI_USER_BOUNDARY: &str = r#"
    ALTER TABLE accounts ADD COLUMN row_version INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE accounts ADD COLUMN created_by TEXT;
    ALTER TABLE accounts ADD COLUMN updated_by TEXT;

    ALTER TABLE transactions ADD COLUMN row_version INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE transactions ADD COLUMN created_by TEXT;
    ALTER TABLE transactions ADD COLUMN updated_by TEXT;

    ALTER TABLE categories ADD COLUMN row_version INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE categories ADD COLUMN created_by TEXT;
    ALTER TABLE categories ADD COLUMN updated_by TEXT;

    ALTER TABLE budgets ADD COLUMN row_version INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE budgets ADD COLUMN created_by TEXT;
    ALTER TABLE budgets ADD COLUMN updated_by TEXT;

    ALTER TABLE savings_goals ADD COLUMN row_version INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE savings_goals ADD COLUMN created_by TEXT;
    ALTER TABLE savings_goals ADD COLUMN updated_by TEXT;

    ALTER TABLE goal_contributions ADD COLUMN row_version INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE goal_contributions ADD COLUMN created_by TEXT;
    ALTER TABLE goal_contributions ADD COLUMN updated_by TEXT;

    ALTER TABLE recurring_transactions ADD COLUMN row_version INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE recurring_transactions ADD COLUMN created_by TEXT;
    ALTER TABLE recurring_transactions ADD COLUMN updated_by TEXT;

    ALTER TABLE category_rules ADD COLUMN row_version INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE category_rules ADD COLUMN created_by TEXT;
    ALTER TABLE category_rules ADD COLUMN updated_by TEXT;

    CREATE TRIGGER IF NOT EXISTS trg_accounts_row_version
    AFTER UPDATE ON accounts FOR EACH ROW
    BEGIN
        UPDATE accounts SET row_version = OLD.row_version + 1 WHERE id = NEW.id;
    END;

    CREATE TRIGGER IF NOT EXISTS trg_transactions_row_version
    AFTER UPDATE ON transactions FOR EACH ROW
    BEGIN
        UPDATE transactions SET row_version = OLD.row_version + 1 WHERE id = NEW.id;
    END;

    CREATE TRIGGER IF NOT EXISTS trg_categories_row_version
    AFTER UPDATE ON categories FOR EACH ROW
    BEGIN
        UPDATE categories SET row_version = OLD.row_version + 1 WHERE id = NEW.id;
    END;

    CREATE TRIGGER IF NOT EXISTS trg_budgets_row_version
    AFTER UPDATE ON budgets FOR EACH ROW
    BEGIN
        UPDATE budgets SET row_version = OLD.row_version + 1 WHERE id = NEW.id;
    END;

    CREATE TRIGGER IF NOT EXISTS trg_savings_goals_row_version
    AFTER UPDATE ON savings_goals FOR EACH ROW
    BEGIN
        UPDATE savings_goals SET row_version = OLD.row_version + 1 WHERE id = NEW.id;
    END;

    CREATE TRIGGER IF NOT EXISTS trg_goal_contributions_row_version
    AFTER UPDATE ON goal_contributions FOR EACH ROW
    BEGIN
        UPDATE goal_contributions SET row_version = OLD.row_version + 1 WHERE id = NEW.id;
    END;

    CREATE TRIGGER IF NOT EXISTS trg_recurring_transactions_row_version
    AFTER UPDATE ON recurring_transactions FOR EACH ROW
    BEGIN
        UPDATE recurring_transactions SET row_version = OLD.row_version + 1 WHERE id = NEW.id;
    END;

    CREATE TRIGGER IF NOT EXISTS trg_category_rules_row_version
    AFTER UPDATE ON category_rules FOR EACH ROW
    BEGIN
        UPDATE category_rules SET row_version = OLD.row_version + 1 WHERE id = NEW.id;
    END;
"#;

// Multi-user phase 2: identities.
//
// An app password is a gate, not a person. These tables are what let the
// boundary answer "who is asking?" rather than merely "may anyone in?".
//
// `login` is UNIQUE COLLATE NOCASE: people type "Sam" one day and "sam" the
// next, and two accounts differing only in case would be a standing trap.
//
// Grants are stored per area, but an owner's grants are never read from these
// rows — see `Actor::new`. An owner with no rows here still reaches everything.
const MIGRATION_011_USERS_AND_GRANTS: &str = r#"
    CREATE TABLE IF NOT EXISTS users (
        id TEXT PRIMARY KEY,
        login TEXT NOT NULL UNIQUE COLLATE NOCASE,
        display_name TEXT NOT NULL,
        password_hash TEXT NOT NULL,
        is_owner INTEGER NOT NULL DEFAULT 0,
        is_active INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        row_version INTEGER NOT NULL DEFAULT 1,
        created_by TEXT,
        updated_by TEXT
    );

    CREATE TABLE IF NOT EXISTS user_grants (
        user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        area TEXT NOT NULL,
        access TEXT NOT NULL,
        PRIMARY KEY (user_id, area)
    );

    CREATE INDEX IF NOT EXISTS idx_user_grants_user ON user_grants(user_id);

    CREATE TRIGGER IF NOT EXISTS trg_users_row_version
    AFTER UPDATE ON users FOR EACH ROW
    BEGIN
        UPDATE users SET row_version = OLD.row_version + 1 WHERE id = NEW.id;
    END;
"#;

// Multi-user phase 3: paired machines.
//
// Two credentials answer two different questions. A device token answers "was
// this machine deliberately added?"; a login and password answer "who is
// sitting at it?". Keeping them separate is what makes the levers safe: a
// stolen laptop can be revoked without anyone changing a password, and someone
// leaving can be deactivated without re-pairing every machine.
//
// Only the SHA-256 of a token is stored. The token itself is written once to
// the machine that paired and never touches the host's disk, so a copy of the
// host database does not yield a working credential.
const MIGRATION_012_PAIRED_DEVICES: &str = r#"
    CREATE TABLE IF NOT EXISTS devices (
        id TEXT PRIMARY KEY,
        label TEXT NOT NULL,
        token_hash TEXT NOT NULL UNIQUE,
        paired_at TEXT NOT NULL,
        last_seen_at TEXT,
        is_revoked INTEGER NOT NULL DEFAULT 0
    );

    CREATE INDEX IF NOT EXISTS idx_devices_token_hash ON devices(token_hash);
"#;
