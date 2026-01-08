export type AccountType = 'checking' | 'savings' | 'credit_card' | 'cash' | 'investment' | 'loan' | 'other';
export type TransactionType = 'income' | 'expense' | 'transfer';
export type TransactionStatus = 'pending' | 'cleared' | 'reconciled' | 'void';
export type CategoryType = 'income' | 'expense';
export type BudgetPeriod = 'weekly' | 'biweekly' | 'monthly' | 'quarterly' | 'yearly';
export type RecurrenceFrequency = 'daily' | 'weekly' | 'biweekly' | 'monthly' | 'quarterly' | 'yearly';
export type GoalType = 'savings' | 'debt_payoff' | 'purchase' | 'emergency' | 'custom';
export type GoalStatus = 'active' | 'completed' | 'paused' | 'cancelled';

export interface Account {
  id: string;
  name: string;
  account_type: AccountType;
  balance: string;
  currency: string;
  institution?: string;
  account_number_last4?: string;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateAccountRequest {
  name: string;
  account_type: AccountType;
  balance?: string;
  currency?: string;
  institution?: string;
  account_number_last4?: string;
}

export interface Transaction {
  id: string;
  account_id: string;
  transaction_type: TransactionType;
  amount: string;
  date: string;
  description: string;
  category_id?: string;
  payee?: string;
  notes?: string;
  status: TransactionStatus;
  is_split: boolean;
  parent_transaction_id?: string;
  recurring_id?: string;
  transfer_account_id?: string;
  imported_id?: string;
  created_at: string;
  updated_at: string;
}

export interface CreateTransactionRequest {
  account_id: string;
  transaction_type: TransactionType;
  amount: string;
  date: string;
  description: string;
  category_id?: string;
  payee?: string;
  notes?: string;
  status?: TransactionStatus;
  transfer_account_id?: string;
}

export interface TransactionFilter {
  account_ids?: string[];
  category_ids?: string[];
  transaction_types?: TransactionType[];
  start_date?: string;
  end_date?: string;
  min_amount?: string;
  max_amount?: string;
  search_text?: string;
  status?: TransactionStatus[];
}

export interface Category {
  id: string;
  name: string;
  category_type: CategoryType;
  color: string;
  icon?: string;
  parent_id?: string;
  is_system: boolean;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateCategoryRequest {
  name: string;
  category_type: CategoryType;
  color: string;
  icon?: string;
  parent_id?: string;
}

export interface Budget {
  id: string;
  name: string;
  category_id: string;
  amount: string;
  period: BudgetPeriod;
  start_date: string;
  end_date?: string;
  rollover: boolean;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateBudgetRequest {
  name: string;
  category_id: string;
  amount: string;
  period: BudgetPeriod;
  start_date: string;
  end_date?: string;
  rollover?: boolean;
}

export interface BudgetStatus {
  budget: Budget;
  category_name: string;
  category_color: string;
  spent: string;
  remaining: string;
  percentage_used: number;
  period_start: string;
  period_end: string;
  is_over_budget: boolean;
}

export interface RecurringTransaction {
  id: string;
  account_id: string;
  transaction_type: TransactionType;
  amount: string;
  description: string;
  category_id?: string;
  payee?: string;
  frequency: RecurrenceFrequency;
  start_date: string;
  end_date?: string;
  next_occurrence: string;
  day_of_month?: number;
  day_of_week?: number;
  auto_post: boolean;
  reminder_days?: number;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateRecurringRequest {
  account_id: string;
  transaction_type: TransactionType;
  amount: string;
  description: string;
  category_id?: string;
  payee?: string;
  frequency: RecurrenceFrequency;
  start_date: string;
  end_date?: string;
  day_of_month?: number;
  auto_post?: boolean;
  reminder_days?: number;
}

export interface UpcomingRecurring {
  recurring: RecurringTransaction;
  next_date: string;
  days_until: number;
  category_name?: string;
  account_name: string;
}

export interface SavingsGoal {
  id: string;
  name: string;
  goal_type: GoalType;
  target_amount: string;
  current_amount: string;
  target_date?: string;
  account_id?: string;
  color: string;
  icon?: string;
  notes?: string;
  status: GoalStatus;
  created_at: string;
  updated_at: string;
}

export interface CreateGoalRequest {
  name: string;
  goal_type: GoalType;
  target_amount: string;
  current_amount?: string;
  target_date?: string;
  account_id?: string;
  color?: string;
  icon?: string;
  notes?: string;
}

export interface CalendarEvent {
  id: string;
  title: string;
  date: string;
  amount: string;
  transaction_type: TransactionType;
  category_name?: string;
  category_color?: string;
  is_recurring: boolean;
  account_name: string;
}

export interface ImportMapping {
  date_column: string;
  description_column: string;
  amount_column: string;
  debit_column?: string;
  credit_column?: string;
  category_column?: string;
  date_format: string;
  has_header: boolean;
  skip_rows: number;
}

export interface RawTransaction {
  date: string;
  description: string;
  amount: string;
  debit?: string;
  credit?: string;
  category?: string;
}

export interface ImportResult {
  imported: Transaction[];
  skipped_duplicates: number;
  errors: string[];
}

export interface SpendingByCategory {
  category_id: string;
  category_name: string;
  category_color: string;
  total: string;
  percentage: number;
  transaction_count: number;
}

export interface MonthlyTrend {
  month: string;
  year: number;
  income: string;
  expenses: string;
  net: string;
}

export interface DailyBalance {
  date: string;
  balance: string;
  income: string;
  expenses: string;
}

export interface CashFlowReport {
  period_start: string;
  period_end: string;
  total_income: string;
  total_expenses: string;
  net_cash_flow: string;
  income_by_category: SpendingByCategory[];
  expenses_by_category: SpendingByCategory[];
  daily_balances: DailyBalance[];
}

export interface CategoryRule {
  id: string;
  category_id: string;
  pattern: string;
  field: string;
  is_regex: boolean;
  priority: number;
  created_at: string;
}

export interface BillReminder {
  recurring_id: string;
  description: string;
  amount: string;
  due_date: string;
  days_until: number;
  transaction_type: TransactionType;
  account_name: string;
  category_name?: string;
}

export interface NotificationSettings {
  enabled: boolean;
  days_before: number;
  show_amount: boolean;
  sound: boolean;
}

export interface EncryptionStatus {
  enabled: boolean;
  unlocked: boolean;
}

export interface BackupMetadata {
  version: string;
  created_at: string;
  app_version: string;
  account_count: number;
  transaction_count: number;
  category_count: number;
}

export interface DetectedRecurring {
  description: string;
  payee?: string;
  frequency: RecurrenceFrequency;
  average_amount: string;
  amount_is_consistent: boolean;
  transaction_type: TransactionType;
  occurrence_count: number;
  occurrence_dates: string[];
  typical_day_of_month?: number;
  confidence: number;
  account_id: string;
  category_id?: string;
}

export interface CancelledSubscription {
  id: string;
  recurring_id: string;
  description: string;
  amount: string;
  frequency: RecurrenceFrequency;
  cancelled_at: string;
  reason?: string;
  estimated_yearly_savings: string;
  created_at: string;
}

export interface SavingsSummary {
  total_yearly_savings: string;
  total_monthly_savings: string;
  cancelled_count: number;
  cancelled_subscriptions: CancelledSubscription[];
}

export interface AutoCategorizeResult {
  total_categorized: number;
  breakdown: CategoryBreakdown[];
}

export interface CategoryBreakdown {
  category_id: string;
  category_name: string;
  count: number;
}
