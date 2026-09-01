# indiBudget — Feature List

**indiBudget** is a privacy-focused, local-first personal finance desktop application. All your financial data lives in an on-device SQLite database — no cloud account, no tracking, no subscription required. Built with a Rust (Tauri) backend and Vue 3 frontend for native performance and a modern interface.

---

## Accounts & Net Worth
- **Seven account types:** Checking, Savings, Credit Card, Cash, Investment, Loan, and Other
- Track opening balance, currency, institution, and masked account number (`****1234`)
- **Always-accurate balances** derived from your transaction history
- **Net Worth tracking** that correctly treats credit cards and loans as liabilities
- **Multi-currency support** with 12 currencies (USD, EUR, GBP, JPY, CAD, AUD, CHF, CNY, INR, MXN, BRL, KRW)
- **Account-to-account transfers** with linked transactions that stay in sync
- **Statement reconciliation** to match your records against bank statements

## Transactions
- Income, expense, and transfer types with category, payee, notes, and status
- **Four-state workflow:** Pending → Cleared → Reconciled, plus Void
- **Powerful filtering:** full-text search, by account, category, type, or date range (with presets)
- **Split transactions** — divide a single transaction across multiple categories
- Live summary stats (income, expenses, net) on any filtered view
- **CSV export** of any transaction set

## Budgeting
- **Five budget periods:** Weekly, Bi-weekly, Monthly, Quarterly, Yearly
- Visual progress bars with over-budget warnings
- Optional **rollover** of unused budget to the next period
- Full **create, edit, and delete** for every budget
- **Budget templates:** 50/30/20 Rule, Zero-Based, Minimalist, and Family Budget — with live preview

## Smart Categorization
- **35+ built-in categories**, fully color-coded, plus unlimited custom categories
- **Subcategories** — organize categories into parent/child groups
- **Intelligent auto-categorization engine** recognizing hundreds of real-world merchants
- **Priority-based matching** with support for text and regex rules
- **Auto-categorize on import** and one-click categorization of existing transactions
- **Batch categorize by keyword** with the option to save reusable rules

## Recurring Transactions & Subscription Management
- Track recurring bills and income across six frequencies
- **Automatic pattern detection** that finds recurring charges in your history with a **confidence score**
- **Subscription cancellation tracking** that calculates your **estimated yearly savings**
- Savings dashboard showing total monthly and annual savings from cancelled subscriptions

## Savings Goals
- **Five goal types:** Savings, Debt Payoff, Purchase, Emergency Fund, Custom
- Progress tracking with target dates and contribution quick-fills
- **Monthly-needed calculator** to stay on pace for time-based goals

## Bill Calendar
- Full month and list calendar views
- Two modes: actual **Transactions** and **Upcoming Bills**
- Daily summaries of bills and deposits, plus monthly expected-income/expense cards
- 30-day upcoming-bills sidebar

## Reports & Analytics
- **Net Worth** summary with 7-day and 30-day change tracking
- **Net Worth History** chart with daily snapshots
- **Cash Flow** analysis with income/expense breakdowns and running balances
- **Spending by Category** doughnut chart
- **Income vs. Expenses** and **Monthly Trends** charts
- **Savings Rate analytics** with best/worst months and a 20%-target indicator
- Year-over-year comparison
- **Export to PDF and CSV**

## Import / Export
- **Five import formats:** CSV, Excel (.xlsx/.xls), OFX, QFX, and QIF (Quicken)
- Automatic format detection and guided column mapping
- **Smart duplicate detection** — re-import the same file safely without creating duplicates
- Auto-categorization of imported transactions
- Import history log

## Bank Sync (SimpleFIN)
- Optional integration with **SimpleFIN**, a privacy-focused bank aggregator
- **Your bank credentials never touch indiBudget** — only a SimpleFIN access token
- Map bank accounts to indiBudget accounts, sync on demand or automatically (daily/weekly)
- Duplicate-aware imports with progress reporting

## Security & Privacy
- **Local-first** — your data lives on hardware you own. There is no cloud account, no server of ours, and nothing is ever sent to us or to any third party.
- With sharing switched off, which is the default, your data never leaves the computer it is on
- With sharing switched on, it travels only between your own computers, over your own network, encrypted with TLS — still never to a cloud service
- Optional **AES-256-GCM encryption** for data at rest
- **Argon2id key derivation** (64 MB / 3 iterations / 256-bit) — resistant to brute-force attacks
- Password strength enforcement, lock/unlock, and password change
- **Path-traversal protection** on all file operations

## Backup & Restore
- **Full JSON backup** of all your data with version checking
- Safe restore that skips duplicates and reports real errors
- **Backup reminders** to keep your data protected

## Notifications
- **Native OS notifications** for upcoming bills
- Configurable reminder window with adaptive messaging ("Due Today," "Due Tomorrow," etc.)

## General
- **Light, Dark, and System themes**
- Configurable currency and date formats
- Dashboard with at-a-glance stats, alerts, and quick insights
- **Payee analytics** — see spending patterns by merchant
- Open source (MIT license)

---

### Technical Highlights
- **Local-first architecture** — on-device SQLite, no cloud dependency
- **Native desktop performance** via Rust + Tauri (small binary, low memory footprint)
- **Bank-grade encryption** (AES-256-GCM + Argon2id) available out of the box
- **Five-format import engine** with idempotent duplicate detection
- **Cross-platform:** Linux (AppImage, .deb, .rpm), with Windows and macOS support via Tauri
