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
  DetectedRecurring,
  CancelledSubscription,
  SavingsSummary,
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
  BillReminder,
  EncryptionStatus,
  BackupMetadata,
  AutoCategorizeResult,
  BatchCategorizeResult,
  UserCategoryRule,
  SplitPart,
} from '../types';

// Initialization
export const initApp = () => invoke<void>('init_app');
export const getDatabasePath = () => invoke<string>('get_database_path');
export const getTransactionCount = () => invoke<number>('get_transaction_count');

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

export const detectRecurringPatterns = () =>
  invoke<DetectedRecurring[]>('detect_recurring_patterns');

export const createRecurringFromDetected = (detected: DetectedRecurring) =>
  invoke<RecurringTransaction>('create_recurring_from_detected', { detected });

export const deactivateRecurring = (id: string, reason?: string) =>
  invoke<CancelledSubscription>('deactivate_recurring', { id, reason });

export const getCancelledSubscriptions = () =>
  invoke<CancelledSubscription[]>('get_cancelled_subscriptions');

export const getSavingsSummary = () =>
  invoke<SavingsSummary>('get_savings_summary');

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

// User Category Rules (custom rules created by batch categorization)
export const getUserCategoryRules = () => invoke<UserCategoryRule[]>('get_user_category_rules');

export const deleteUserCategoryRule = (ruleId: string) =>
  invoke<boolean>('delete_user_category_rule', { ruleId });

// Auto-Categorize
export const autoCategorizeTransactions = () =>
  invoke<AutoCategorizeResult>('auto_categorize_transactions');

// Batch Categorize - also saves the keyword as a rule for future auto-categorization
export const batchCategorizeTransactions = (keyword: string, categoryId: string, matchUncategorizedOnly: boolean, saveRule: boolean = true) =>
  invoke<BatchCategorizeResult>('batch_categorize_transactions', { keyword, categoryId, matchUncategorizedOnly, saveRule });

// Notifications
export const getBillReminders = (daysAhead?: number) =>
  invoke<BillReminder[]>('get_bill_reminders', { daysAhead });

export const sendBillNotification = (title: string, body: string) =>
  invoke<void>('send_bill_notification', { title, body });

export const checkAndSendNotifications = (daysBefore: number, showAmount: boolean) =>
  invoke<number>('check_and_send_notifications', { daysBefore, showAmount });

// Encryption
export const getEncryptionStatus = () =>
  invoke<EncryptionStatus>('get_encryption_status');

export const enableEncryption = (password: string) =>
  invoke<void>('enable_encryption', { password });

export const disableEncryption = (password: string) =>
  invoke<void>('disable_encryption', { password });

export const unlockEncryption = (password: string) =>
  invoke<void>('unlock_encryption', { password });

export const lockEncryption = () =>
  invoke<void>('lock_encryption');

export const changeEncryptionPassword = (oldPassword: string, newPassword: string) =>
  invoke<void>('change_encryption_password', { oldPassword, newPassword });

// Backup
export const exportBackup = (path: string) =>
  invoke<BackupMetadata>('export_backup', { path });

export const importBackup = (path: string) =>
  invoke<BackupMetadata>('import_backup', { path });

export const getBackupInfo = (path: string) =>
  invoke<BackupMetadata>('get_backup_info', { path });

export const getDefaultBackupPath = () =>
  invoke<string>('get_default_backup_path');

// Split Transactions
export const createSplitTransaction = async (
  parentTransactionId: string,
  parts: SplitPart[]
): Promise<Transaction[]> => {
  // Get the parent transaction
  const parent = await getTransaction(parentTransactionId);

  // Mark parent as split
  await updateTransaction({
    id: parentTransactionId,
    is_split: true,
  });

  // Create child transactions for each split part
  const childTransactions: Transaction[] = [];
  for (const part of parts) {
    const child = await invoke<Transaction>('create_transaction', {
      request: {
        account_id: parent.account_id,
        transaction_type: parent.transaction_type,
        amount: part.amount,
        date: parent.date,
        description: part.description || parent.description,
        category_id: part.category_id,
        payee: parent.payee,
        status: parent.status,
        parent_transaction_id: parentTransactionId,
      },
    });
    childTransactions.push(child);
  }

  return childTransactions;
};

export const getSplitParts = async (parentTransactionId: string): Promise<Transaction[]> => {
  const allTransactions = await getTransactions({});
  return allTransactions.filter(t => t.parent_transaction_id === parentTransactionId);
};

export const unsplitTransaction = async (parentTransactionId: string): Promise<void> => {
  // Get and delete all child transactions
  const children = await getSplitParts(parentTransactionId);
  for (const child of children) {
    await deleteTransaction(child.id);
  }

  // Mark parent as not split
  await updateTransaction({
    id: parentTransactionId,
    is_split: false,
  });
};

// SimpleFIN Integration - Import a single transaction with duplicate detection
export const importSingleTransaction = async (
  request: CreateTransactionRequest,
  importedId: string
): Promise<{ imported: boolean; duplicate: boolean; transaction?: Transaction }> => {
  // Check for existing transaction with same imported_id in notes
  // or same date + description + amount + account (fallback)
  const existingTransactions = await getTransactions({
    account_ids: [request.account_id],
    start_date: request.date,
    end_date: request.date,
  });

  // Check for duplicate by looking for the imported_id marker in notes
  const isDuplicate = existingTransactions.some(tx => {
    // Check if notes contain the imported_id
    if (tx.notes?.includes(importedId)) {
      return true;
    }
    // Fallback: check if same date, amount, and similar description
    if (
      tx.date === request.date &&
      Math.abs(parseFloat(tx.amount) - parseFloat(request.amount)) < 0.01 &&
      tx.description.toLowerCase().includes(request.description.toLowerCase().substring(0, 20))
    ) {
      return true;
    }
    return false;
  });

  if (isDuplicate) {
    return { imported: false, duplicate: true };
  }

  // Add the imported_id to notes for future duplicate detection
  const notesWithId = request.notes
    ? `${request.notes} [ID:${importedId}]`
    : `[ID:${importedId}]`;

  const transaction = await createTransaction({
    ...request,
    notes: notesWithId,
  });

  return { imported: true, duplicate: false, transaction };
};
