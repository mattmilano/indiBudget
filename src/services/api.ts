import { invoke } from '@tauri-apps/api/core';
import type {
  Account,
  CreateAccountRequest,
  Transaction,
  CreateTransactionRequest,
  TransactionFilter,
  Category,
  CreateCategoryRequest,
  Budget,
  CreateBudgetRequest,
  BudgetStatus,
  RecurringTransaction,
  CreateRecurringRequest,
  UpcomingRecurring,
  SavingsGoal,
  CreateGoalRequest,
  CalendarEvent,
  ImportMapping,
  RawTransaction,
  ImportResult,
  SpendingByCategory,
  MonthlyTrend,
  CashFlowReport,
  CategoryRule,
} from '../types';

// Initialization
export const initApp = () => invoke<void>('init_app');

// Accounts
export const createAccount = (request: CreateAccountRequest) =>
  invoke<Account>('create_account', { request });

export const getAccounts = () => invoke<Account[]>('get_accounts');

export const getAccount = (id: string) => invoke<Account>('get_account', { id });

export const updateAccount = (request: Partial<Account> & { id: string }) =>
  invoke<Account>('update_account', { request });

export const deleteAccount = (id: string) => invoke<void>('delete_account', { id });

// Transactions
export const createTransaction = (request: CreateTransactionRequest) =>
  invoke<Transaction>('create_transaction', { request });

export const getTransactions = (filter: TransactionFilter = {}) =>
  invoke<Transaction[]>('get_transactions', { filter });

export const getTransaction = (id: string) => invoke<Transaction>('get_transaction', { id });

export const updateTransaction = (request: Partial<Transaction> & { id: string }) =>
  invoke<Transaction>('update_transaction', { request });

export const deleteTransaction = (id: string) => invoke<void>('delete_transaction', { id });

// Categories
export const getCategories = () => invoke<Category[]>('get_categories');

export const createCategory = (request: CreateCategoryRequest) =>
  invoke<Category>('create_category', { request });

// Budgets
export const createBudget = (request: CreateBudgetRequest) =>
  invoke<Budget>('create_budget', { request });

export const getBudgets = () => invoke<Budget[]>('get_budgets');

export const getBudgetStatus = (asOfDate?: string) =>
  invoke<BudgetStatus[]>('get_budget_status', { asOfDate });

// Recurring Transactions
export const createRecurring = (request: CreateRecurringRequest) =>
  invoke<RecurringTransaction>('create_recurring', { request });

export const getRecurring = () => invoke<RecurringTransaction[]>('get_recurring');

export const getUpcomingRecurring = (days?: number) =>
  invoke<UpcomingRecurring[]>('get_upcoming_recurring', { days });

// Goals
export const createGoal = (request: CreateGoalRequest) =>
  invoke<SavingsGoal>('create_goal', { request });

export const getGoals = () => invoke<SavingsGoal[]>('get_goals');

export const updateGoalProgress = (id: string, amount: string) =>
  invoke<void>('update_goal_progress', { id, amount });

// Import
export const detectImportColumns = (path: string) =>
  invoke<string[]>('detect_import_columns', { path });

export const previewImport = (path: string, mapping: ImportMapping) =>
  invoke<RawTransaction[]>('preview_import', { path, mapping });

export const importTransactions = (path: string, accountId: string, mapping: ImportMapping) =>
  invoke<ImportResult>('import_transactions', { path, accountId, mapping });

// Reports
export const getSpendingByCategory = (startDate?: string, endDate?: string) =>
  invoke<SpendingByCategory[]>('get_spending_by_category', { startDate, endDate });

export const getMonthlyTrends = (months?: number) =>
  invoke<MonthlyTrend[]>('get_monthly_trends', { months });

export const getCashFlowReport = (startDate: string, endDate: string) =>
  invoke<CashFlowReport>('get_cash_flow_report', { startDate, endDate });

// Calendar
export const getCalendarEvents = (startDate: string, endDate: string) =>
  invoke<CalendarEvent[]>('get_calendar_events', { startDate, endDate });

// Category Rules
export const createCategoryRule = (categoryId: string, pattern: string, field?: string) =>
  invoke<CategoryRule>('create_category_rule', { categoryId, pattern, field });

export const getCategoryRules = () => invoke<CategoryRule[]>('get_category_rules');
