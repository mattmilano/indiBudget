<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue';
import { useTransactionsStore, useAccountsStore, useCategoriesStore } from '../stores';
import type { CreateTransactionRequest, TransactionType, AutoCategorizeResult, BatchCategorizeResult, UserCategoryRule } from '../types';
import { format, startOfMonth, endOfMonth, startOfQuarter, endOfQuarter, startOfYear, endOfYear, subMonths, subYears } from 'date-fns';
import * as api from '../services/api';

const transactionsStore = useTransactionsStore();
const accountsStore = useAccountsStore();
const categoriesStore = useCategoriesStore();

const showAddModal = ref(false);
const showCategorizeResultModal = ref(false);
const categorizing = ref(false);
const categorizeResult = ref<AutoCategorizeResult | null>(null);
const searchQuery = ref('');
const filterAccountId = ref('');
const filterCategoryId = ref('');
const filterType = ref<TransactionType | ''>('');

// Batch categorization state
const showBatchModal = ref(false);
const batchKeyword = ref('');
const batchCategoryId = ref('');
const batchUncategorizedOnly = ref(true);
const batchProcessing = ref(false);
const batchResult = ref<BatchCategorizeResult | null>(null);

// User rules management state
const showRulesModal = ref(false);
const userRules = ref<UserCategoryRule[]>([]);
const loadingRules = ref(false);
const deletingRuleId = ref<string | null>(null);

// Pagination state
const currentPage = ref(1);
const pageSize = ref(50);
const pageSizeOptions = [25, 50, 100, 200];

// Date range filters
type DatePreset = 'all' | 'this-month' | 'last-month' | 'this-quarter' | 'this-year' | 'last-year' | 'custom';
const datePreset = ref<DatePreset>('all');
const customStartDate = ref('');
const customEndDate = ref('');

// Calculate date ranges based on preset
const dateRange = computed(() => {
  const now = new Date();

  switch (datePreset.value) {
    case 'this-month':
      return {
        start: format(startOfMonth(now), 'yyyy-MM-dd'),
        end: format(endOfMonth(now), 'yyyy-MM-dd'),
      };
    case 'last-month': {
      const lastMonth = subMonths(now, 1);
      return {
        start: format(startOfMonth(lastMonth), 'yyyy-MM-dd'),
        end: format(endOfMonth(lastMonth), 'yyyy-MM-dd'),
      };
    }
    case 'this-quarter':
      return {
        start: format(startOfQuarter(now), 'yyyy-MM-dd'),
        end: format(endOfQuarter(now), 'yyyy-MM-dd'),
      };
    case 'this-year':
      return {
        start: format(startOfYear(now), 'yyyy-MM-dd'),
        end: format(endOfYear(now), 'yyyy-MM-dd'),
      };
    case 'last-year': {
      const lastYear = subYears(now, 1);
      return {
        start: format(startOfYear(lastYear), 'yyyy-MM-dd'),
        end: format(endOfYear(lastYear), 'yyyy-MM-dd'),
      };
    }
    case 'custom':
      return {
        start: customStartDate.value || '',
        end: customEndDate.value || '',
      };
    case 'all':
    default:
      return { start: '', end: '' };
  }
});

const newTransaction = ref<CreateTransactionRequest>({
  account_id: '',
  transaction_type: 'expense',
  amount: '',
  date: format(new Date(), 'yyyy-MM-dd'),
  description: '',
  category_id: undefined,
  payee: undefined,
  notes: undefined,
});

const filteredTransactions = computed(() => {
  let result = transactionsStore.sortedTransactions;

  // Apply date range filter
  if (dateRange.value.start) {
    result = result.filter(t => t.date >= dateRange.value.start);
  }
  if (dateRange.value.end) {
    result = result.filter(t => t.date <= dateRange.value.end);
  }

  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase();
    result = result.filter(
      t =>
        t.description.toLowerCase().includes(query) ||
        t.payee?.toLowerCase().includes(query)
    );
  }

  if (filterAccountId.value) {
    result = result.filter(t => t.account_id === filterAccountId.value);
  }

  if (filterCategoryId.value) {
    if (filterCategoryId.value === '__uncategorized__') {
      result = result.filter(t => !t.category_id);
    } else {
      result = result.filter(t => t.category_id === filterCategoryId.value);
    }
  }

  if (filterType.value) {
    result = result.filter(t => t.transaction_type === filterType.value);
  }

  return result;
});

// Pagination computed properties
const totalPages = computed(() => Math.ceil(filteredTransactions.value.length / pageSize.value));

const paginatedTransactions = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value;
  const end = start + pageSize.value;
  return filteredTransactions.value.slice(start, end);
});

// Reset to page 1 when filters change
function resetPagination() {
  currentPage.value = 1;
}

// Watch for filter changes to reset pagination
const goToPage = (page: number) => {
  if (page >= 1 && page <= totalPages.value) {
    currentPage.value = page;
    // Scroll to top of transaction list
    window.scrollTo({ top: 0, behavior: 'smooth' });
  }
};

// Reset pagination when filters change
watch([searchQuery, filterAccountId, filterCategoryId, filterType, datePreset, customStartDate, customEndDate, pageSize], () => {
  currentPage.value = 1;
});

// Summary stats for filtered transactions
const transactionSummary = computed(() => {
  let income = 0;
  let expenses = 0;

  for (const tx of filteredTransactions.value) {
    const amount = parseFloat(tx.amount) || 0;
    if (tx.transaction_type === 'income') {
      income += amount;
    } else if (tx.transaction_type === 'expense') {
      expenses += amount;
    }
  }

  return {
    count: filteredTransactions.value.length,
    income,
    expenses,
    net: income - expenses,
  };
});

const formatCurrency = (value: string | number) => {
  const num = typeof value === 'string' ? parseFloat(value) : value;
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(num || 0);
};

async function handleSubmit() {
  try {
    await transactionsStore.createTransaction(newTransaction.value);
    showAddModal.value = false;
    resetForm();
  } catch (e) {
    console.error('Failed to create transaction:', e);
  }
}

function resetForm() {
  newTransaction.value = {
    account_id: accountsStore.accounts[0]?.id || '',
    transaction_type: 'expense',
    amount: '',
    date: format(new Date(), 'yyyy-MM-dd'),
    description: '',
    category_id: undefined,
    payee: undefined,
    notes: undefined,
  };
}

async function deleteTransaction(id: string) {
  if (confirm('Are you sure you want to delete this transaction?')) {
    await transactionsStore.deleteTransaction(id);
  }
}

async function autoCategorize() {
  categorizing.value = true;
  try {
    const result = await api.autoCategorizeTransactions();
    categorizeResult.value = result;
    if (result.total_categorized > 0) {
      // Refresh transactions to show updated categories
      await transactionsStore.fetchTransactions();
      showCategorizeResultModal.value = true;
    } else {
      alert('All transactions are already categorized. Nothing to do!');
    }
  } catch (e) {
    console.error('Failed to auto-categorize:', e);
    alert('Failed to auto-categorize transactions.');
  } finally {
    categorizing.value = false;
  }
}

const uncategorizedCount = computed(() => {
  return filteredTransactions.value.filter(t => !t.category_id).length;
});

// Preview matching transactions for batch categorization
const batchPreviewTransactions = computed(() => {
  if (!batchKeyword.value.trim()) return [];
  const keyword = batchKeyword.value.toLowerCase();
  return transactionsStore.sortedTransactions.filter(t => {
    // Check uncategorized filter
    if (batchUncategorizedOnly.value && t.category_id) return false;
    // Check keyword match
    const descMatch = t.description.toLowerCase().includes(keyword);
    const payeeMatch = t.payee?.toLowerCase().includes(keyword) || false;
    return descMatch || payeeMatch;
  });
});

async function executeBatchCategorize() {
  if (!batchKeyword.value.trim() || !batchCategoryId.value) {
    alert('Please enter a keyword and select a category.');
    return;
  }

  batchProcessing.value = true;
  try {
    const result = await api.batchCategorizeTransactions(
      batchKeyword.value,
      batchCategoryId.value,
      batchUncategorizedOnly.value
    );
    batchResult.value = result;

    if (result.total_updated > 0) {
      // Refresh transactions to show updated categories
      await transactionsStore.fetchTransactions({});
      const ruleMsg = result.rule_saved
        ? '\n\nThis keyword has been saved and will be used to automatically categorize future imports.'
        : '';
      alert(`Successfully categorized ${result.total_updated} transactions matching "${result.keyword}".${ruleMsg}`);
      closeBatchModal();
    } else {
      alert('No matching transactions found to categorize.');
    }
  } catch (e) {
    console.error('Failed to batch categorize:', e);
    alert('Failed to batch categorize transactions.');
  } finally {
    batchProcessing.value = false;
  }
}

function closeBatchModal() {
  showBatchModal.value = false;
  batchKeyword.value = '';
  batchCategoryId.value = '';
  batchUncategorizedOnly.value = true;
  batchResult.value = null;
}

// User rules management functions
async function loadUserRules() {
  loadingRules.value = true;
  try {
    userRules.value = await api.getUserCategoryRules();
  } catch (e) {
    console.error('Failed to load user rules:', e);
  } finally {
    loadingRules.value = false;
  }
}

async function openRulesModal() {
  showRulesModal.value = true;
  await loadUserRules();
}

async function deleteUserRule(ruleId: string) {
  if (!confirm('Are you sure you want to delete this rule? Future imports will no longer use this categorization.')) {
    return;
  }

  deletingRuleId.value = ruleId;
  try {
    await api.deleteUserCategoryRule(ruleId);
    userRules.value = userRules.value.filter(r => r.id !== ruleId);
  } catch (e) {
    console.error('Failed to delete rule:', e);
    alert('Failed to delete the rule.');
  } finally {
    deletingRuleId.value = null;
  }
}

onMounted(async () => {
  await Promise.all([
    accountsStore.fetchAccounts(),
    categoriesStore.fetchCategories(),
    // Explicitly pass empty filter to fetch ALL transactions
    // (Dashboard may have set a month filter that we need to clear)
    transactionsStore.fetchTransactions({}),
  ]);
  if (accountsStore.accounts.length > 0) {
    newTransaction.value.account_id = accountsStore.accounts[0].id;
  }
});
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Transactions</h1>
      <div class="flex gap-3">
        <button
          v-if="uncategorizedCount > 0"
          @click="autoCategorize"
          :disabled="categorizing"
          class="px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 transition-colors disabled:opacity-50 flex items-center gap-2"
        >
          <svg v-if="categorizing" class="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
          </svg>
          <svg v-else class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z" />
          </svg>
          {{ categorizing ? 'Categorizing...' : `Auto-Categorize (${uncategorizedCount})` }}
        </button>
        <button
          @click="openRulesModal"
          class="px-4 py-2 border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors flex items-center gap-2"
          title="Manage custom categorization rules"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
          Rules
        </button>
        <button
          @click="showAddModal = true"
          class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          Add Transaction
        </button>
      </div>
    </div>

    <!-- Date Range Selector -->
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4 mb-4">
      <div class="flex flex-wrap items-center gap-3">
        <span class="text-sm font-medium text-gray-700 dark:text-gray-300">Date Range:</span>
        <div class="flex flex-wrap gap-2">
          <button
            v-for="preset in ['all', 'this-month', 'last-month', 'this-quarter', 'this-year', 'last-year'] as DatePreset[]"
            :key="preset"
            @click="datePreset = preset"
            :class="[
              'px-3 py-1.5 text-sm rounded-lg transition-colors',
              datePreset === preset
                ? 'bg-blue-600 text-white'
                : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600'
            ]"
          >
            {{ preset === 'all' ? 'All Time' :
               preset === 'this-month' ? 'This Month' :
               preset === 'last-month' ? 'Last Month' :
               preset === 'this-quarter' ? 'This Quarter' :
               preset === 'this-year' ? 'This Year' :
               'Last Year' }}
          </button>
          <button
            @click="datePreset = 'custom'"
            :class="[
              'px-3 py-1.5 text-sm rounded-lg transition-colors',
              datePreset === 'custom'
                ? 'bg-blue-600 text-white'
                : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600'
            ]"
          >
            Custom
          </button>
        </div>

        <!-- Custom Date Inputs -->
        <div v-if="datePreset === 'custom'" class="flex items-center gap-2 ml-auto">
          <input
            v-model="customStartDate"
            type="date"
            class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
          />
          <span class="text-gray-500">to</span>
          <input
            v-model="customEndDate"
            type="date"
            class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
          />
        </div>
      </div>
    </div>

    <!-- Summary Stats -->
    <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mb-4">
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
        <p class="text-sm text-gray-500 dark:text-gray-400">Transactions</p>
        <p class="text-2xl font-bold text-gray-900 dark:text-white">{{ transactionSummary.count }}</p>
      </div>
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
        <p class="text-sm text-gray-500 dark:text-gray-400">Income</p>
        <p class="text-2xl font-bold text-green-600">{{ formatCurrency(transactionSummary.income) }}</p>
      </div>
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
        <p class="text-sm text-gray-500 dark:text-gray-400">Expenses</p>
        <p class="text-2xl font-bold text-red-600">{{ formatCurrency(transactionSummary.expenses) }}</p>
      </div>
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
        <p class="text-sm text-gray-500 dark:text-gray-400">Net</p>
        <p :class="['text-2xl font-bold', transactionSummary.net >= 0 ? 'text-green-600' : 'text-red-600']">
          {{ formatCurrency(transactionSummary.net) }}
        </p>
      </div>
    </div>

    <!-- Filters -->
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4 mb-6">
      <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
        <input
          v-model="searchQuery"
          type="text"
          placeholder="Search transactions..."
          class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
        />
        <select
          v-model="filterAccountId"
          class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
        >
          <option value="">All Accounts</option>
          <option v-for="account in accountsStore.accounts" :key="account.id" :value="account.id">
            {{ account.name }}
          </option>
        </select>
        <select
          v-model="filterCategoryId"
          class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
        >
          <option value="">All Categories</option>
          <option value="__uncategorized__">Uncategorized</option>
          <option v-for="category in categoriesStore.categories" :key="category.id" :value="category.id">
            {{ category.name }}
          </option>
        </select>
        <select
          v-model="filterType"
          class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
        >
          <option value="">All Types</option>
          <option value="income">Income</option>
          <option value="expense">Expense</option>
          <option value="transfer">Transfer</option>
        </select>
      </div>
    </div>

    <!-- Transactions List -->
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
      <div v-if="filteredTransactions.length === 0" class="p-8 text-center text-gray-500">
        No transactions found for the selected period. Try adjusting your filters or date range.
      </div>
      <div v-else class="divide-y divide-gray-200 dark:divide-gray-700">
        <div
          v-for="tx in paginatedTransactions"
          :key="tx.id"
          class="p-4 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
        >
          <div class="flex items-center justify-between">
            <div class="flex-1">
              <div class="flex items-center gap-3">
                <div
                  class="w-10 h-10 rounded-full flex items-center justify-center"
                  :class="tx.transaction_type === 'income' ? 'bg-green-100 text-green-600' : 'bg-red-100 text-red-600'"
                >
                  <svg v-if="tx.transaction_type === 'income'" class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
                  </svg>
                  <svg v-else class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20 12H4" />
                  </svg>
                </div>
                <div>
                  <p class="font-medium text-gray-900 dark:text-white">{{ tx.description }}</p>
                  <p class="text-sm text-gray-500 dark:text-gray-400">
                    {{ tx.date }} · {{ accountsStore.accountsById[tx.account_id]?.name || 'Unknown Account' }}
                    <span v-if="tx.category_id" class="ml-2">
                      · {{ categoriesStore.getCategoryById(tx.category_id)?.name }}
                    </span>
                  </p>
                </div>
              </div>
            </div>
            <div class="flex items-center gap-4">
              <p
                :class="[
                  'text-lg font-semibold',
                  tx.transaction_type === 'income' ? 'text-green-600' : 'text-red-600'
                ]"
              >
                {{ tx.transaction_type === 'income' ? '+' : '-' }}{{ formatCurrency(tx.amount) }}
              </p>
              <button
                @click="deleteTransaction(tx.id)"
                class="p-2 text-gray-400 hover:text-red-600 transition-colors"
              >
                <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Pagination Controls -->
    <div v-if="filteredTransactions.length > 0" class="bg-white dark:bg-gray-800 rounded-lg shadow mt-4 p-4">
      <div class="flex flex-col sm:flex-row items-center justify-between gap-4">
        <!-- Results info -->
        <div class="text-sm text-gray-500 dark:text-gray-400">
          Showing {{ (currentPage - 1) * pageSize + 1 }} - {{ Math.min(currentPage * pageSize, filteredTransactions.length) }}
          of {{ filteredTransactions.length.toLocaleString() }} transactions
        </div>

        <!-- Page controls -->
        <div class="flex items-center gap-2">
          <!-- Page size selector -->
          <select
            v-model="pageSize"
            class="px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
          >
            <option v-for="size in pageSizeOptions" :key="size" :value="size">{{ size }} per page</option>
          </select>

          <!-- Previous button -->
          <button
            @click="goToPage(currentPage - 1)"
            :disabled="currentPage === 1"
            class="px-3 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Previous
          </button>

          <!-- Page numbers -->
          <div class="flex items-center gap-1">
            <button
              v-if="currentPage > 2"
              @click="goToPage(1)"
              class="px-3 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-700"
            >
              1
            </button>
            <span v-if="currentPage > 3" class="px-2 text-gray-400">...</span>

            <template v-for="page in Math.min(5, totalPages)" :key="page">
              <button
                v-if="Math.abs(page + Math.max(0, currentPage - 3) - currentPage) <= 1 || totalPages <= 5"
                @click="goToPage(totalPages <= 5 ? page : page + Math.max(0, currentPage - 3))"
                :class="[
                  'px-3 py-1 text-sm border rounded',
                  (totalPages <= 5 ? page : page + Math.max(0, currentPage - 3)) === currentPage
                    ? 'bg-blue-600 text-white border-blue-600'
                    : 'border-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700'
                ]"
              >
                {{ totalPages <= 5 ? page : page + Math.max(0, currentPage - 3) }}
              </button>
            </template>

            <span v-if="currentPage < totalPages - 2 && totalPages > 5" class="px-2 text-gray-400">...</span>
            <button
              v-if="currentPage < totalPages - 1 && totalPages > 5"
              @click="goToPage(totalPages)"
              class="px-3 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-700"
            >
              {{ totalPages }}
            </button>
          </div>

          <!-- Next button -->
          <button
            @click="goToPage(currentPage + 1)"
            :disabled="currentPage === totalPages"
            class="px-3 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Next
          </button>
        </div>
      </div>
    </div>

    <!-- Add Transaction Modal -->
    <div
      v-if="showAddModal"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showAddModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Add Transaction</h3>
        </div>
        <form @submit.prevent="handleSubmit" class="p-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Type</label>
            <select
              v-model="newTransaction.transaction_type"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            >
              <option value="expense">Expense</option>
              <option value="income">Income</option>
              <option value="transfer">Transfer</option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Account</label>
            <select
              v-model="newTransaction.account_id"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            >
              <option v-for="account in accountsStore.accounts" :key="account.id" :value="account.id">
                {{ account.name }}
              </option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Amount</label>
            <input
              v-model="newTransaction.amount"
              type="number"
              step="0.01"
              min="0"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Date</label>
            <input
              v-model="newTransaction.date"
              type="date"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Description</label>
            <input
              v-model="newTransaction.description"
              type="text"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Category</label>
            <select
              v-model="newTransaction.category_id"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            >
              <option value="">Uncategorized</option>
              <optgroup label="Income" v-if="newTransaction.transaction_type === 'income'">
                <option v-for="cat in categoriesStore.incomeCategories" :key="cat.id" :value="cat.id">
                  {{ cat.name }}
                </option>
              </optgroup>
              <optgroup label="Expenses" v-else>
                <option v-for="cat in categoriesStore.expenseCategories" :key="cat.id" :value="cat.id">
                  {{ cat.name }}
                </option>
              </optgroup>
            </select>
          </div>
          <div class="flex justify-end gap-3 pt-4">
            <button
              type="button"
              @click="showAddModal = false"
              class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
            >
              Add Transaction
            </button>
          </div>
        </form>
      </div>
    </div>

    <!-- Auto-Categorize Result Modal -->
    <div
      v-if="showCategorizeResultModal && categorizeResult"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showCategorizeResultModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4 max-h-[80vh] flex flex-col">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center gap-3 flex-shrink-0">
          <div class="w-10 h-10 bg-green-100 dark:bg-green-900 rounded-full flex items-center justify-center">
            <svg class="w-5 h-5 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
            </svg>
          </div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Categorization Complete</h3>
        </div>
        <div class="p-4 overflow-y-auto flex-1">
          <p class="text-gray-700 dark:text-gray-300 mb-4">
            Successfully categorized <strong>{{ categorizeResult.total_categorized }}</strong> transactions!
          </p>
          <div v-if="categorizeResult.breakdown.length > 0" class="space-y-2">
            <h4 class="text-sm font-medium text-gray-500 dark:text-gray-400">Breakdown by category:</h4>
            <div
              v-for="item in categorizeResult.breakdown"
              :key="item.category_id"
              class="flex justify-between items-center py-2 px-3 bg-gray-50 dark:bg-gray-700 rounded-lg"
            >
              <span class="text-gray-900 dark:text-white">{{ item.category_name }}</span>
              <span class="text-sm font-medium text-gray-600 dark:text-gray-400">{{ item.count }} transactions</span>
            </div>
          </div>
        </div>
        <div class="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end flex-shrink-0">
          <button
            @click="showCategorizeResultModal = false"
            class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
          >
            Done
          </button>
        </div>
      </div>
    </div>

    <!-- Batch Categorize Modal -->
    <div
      v-if="showBatchModal"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50"
      @click.self="closeBatchModal"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-2xl max-h-[80vh] overflow-hidden">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center gap-3">
          <div class="p-2 bg-indigo-100 dark:bg-indigo-900 rounded-lg">
            <svg class="w-5 h-5 text-indigo-600 dark:text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
            </svg>
          </div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Batch Categorize Transactions</h3>
        </div>
        <div class="p-4 space-y-4">
          <p class="text-sm text-gray-600 dark:text-gray-400">
            Find transactions by keyword and assign them to a category. This is useful for correcting mis-categorized transactions.
          </p>

          <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Keyword to match
              </label>
              <input
                v-model="batchKeyword"
                type="text"
                placeholder="e.g., Capital One, Kroger Fuel"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
              />
            </div>
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Assign to category
              </label>
              <select
                v-model="batchCategoryId"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
              >
                <option value="">Select a category...</option>
                <option v-for="category in categoriesStore.categories" :key="category.id" :value="category.id">
                  {{ category.name }}
                </option>
              </select>
            </div>
          </div>

          <div class="flex items-center gap-2">
            <input
              id="uncategorized-only"
              v-model="batchUncategorizedOnly"
              type="checkbox"
              class="w-4 h-4 text-indigo-600 border-gray-300 rounded focus:ring-indigo-500"
            />
            <label for="uncategorized-only" class="text-sm text-gray-700 dark:text-gray-300">
              Only update uncategorized transactions (uncheck to re-categorize all matching)
            </label>
          </div>

          <!-- Preview -->
          <div v-if="batchKeyword.trim()" class="border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
            <div class="bg-gray-50 dark:bg-gray-700 px-3 py-2 border-b border-gray-200 dark:border-gray-600">
              <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
                Preview: {{ batchPreviewTransactions.length }} matching transaction(s)
              </span>
            </div>
            <div class="max-h-48 overflow-y-auto">
              <div v-if="batchPreviewTransactions.length === 0" class="p-4 text-center text-gray-500 text-sm">
                No matching transactions found.
              </div>
              <div v-else>
                <div
                  v-for="tx in batchPreviewTransactions.slice(0, 10)"
                  :key="tx.id"
                  class="px-3 py-2 border-b border-gray-100 dark:border-gray-700 last:border-0 text-sm"
                >
                  <div class="flex justify-between items-center">
                    <span class="text-gray-900 dark:text-white truncate">{{ tx.description }}</span>
                    <span :class="tx.transaction_type === 'income' ? 'text-green-600' : 'text-red-600'">
                      {{ tx.transaction_type === 'income' ? '+' : '-' }}${{ parseFloat(tx.amount).toFixed(2) }}
                    </span>
                  </div>
                  <div class="text-xs text-gray-500">{{ tx.date }}</div>
                </div>
                <div v-if="batchPreviewTransactions.length > 10" class="px-3 py-2 text-center text-sm text-gray-500">
                  ... and {{ batchPreviewTransactions.length - 10 }} more
                </div>
              </div>
            </div>
          </div>
        </div>
        <div class="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-3">
          <button
            @click="closeBatchModal"
            class="px-4 py-2 border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
          >
            Cancel
          </button>
          <button
            @click="executeBatchCategorize"
            :disabled="!batchKeyword.trim() || !batchCategoryId || batchProcessing || batchPreviewTransactions.length === 0"
            class="px-4 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
          >
            <svg v-if="batchProcessing" class="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            {{ batchProcessing ? 'Applying...' : `Apply to ${batchPreviewTransactions.length} Transaction(s)` }}
          </button>
        </div>
      </div>
    </div>

    <!-- User Rules Management Modal -->
    <div
      v-if="showRulesModal"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50"
      @click.self="showRulesModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-lg max-h-[80vh] overflow-hidden">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
          <div class="flex items-center gap-3">
            <div class="p-2 bg-gray-100 dark:bg-gray-700 rounded-lg">
              <svg class="w-5 h-5 text-gray-600 dark:text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
              </svg>
            </div>
            <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Custom Categorization Rules</h3>
          </div>
          <button
            @click="showRulesModal = false"
            class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
          >
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <div class="p-4">
          <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
            These rules were created when you batch-categorized transactions. They are automatically applied to future imports.
          </p>

          <!-- Loading state -->
          <div v-if="loadingRules" class="py-8 text-center text-gray-500">
            <svg class="animate-spin h-6 w-6 mx-auto mb-2" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            Loading rules...
          </div>

          <!-- Empty state -->
          <div v-else-if="userRules.length === 0" class="py-8 text-center">
            <svg class="w-12 h-12 mx-auto text-gray-300 dark:text-gray-600 mb-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
            </svg>
            <p class="text-gray-500 dark:text-gray-400">No custom rules yet.</p>
            <p class="text-sm text-gray-400 dark:text-gray-500 mt-1">Use "Batch Categorize" to create rules.</p>
          </div>

          <!-- Rules list -->
          <div v-else class="space-y-2 max-h-72 overflow-y-auto">
            <div
              v-for="rule in userRules"
              :key="rule.id"
              class="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-700 rounded-lg"
            >
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="font-medium text-gray-900 dark:text-white truncate">"{{ rule.pattern }}"</span>
                  <svg class="w-4 h-4 text-gray-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 7l5 5m0 0l-5 5m5-5H6" />
                  </svg>
                  <span class="text-indigo-600 dark:text-indigo-400 truncate">{{ rule.category_name }}</span>
                </div>
              </div>
              <button
                @click="deleteUserRule(rule.id)"
                :disabled="deletingRuleId === rule.id"
                class="ml-3 p-1.5 text-gray-400 hover:text-red-600 dark:hover:text-red-400 transition-colors disabled:opacity-50"
                title="Delete rule"
              >
                <svg v-if="deletingRuleId === rule.id" class="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
                  <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                  <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                </svg>
                <svg v-else class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
              </button>
            </div>
          </div>
        </div>
        <div class="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end">
          <button
            @click="showRulesModal = false"
            class="px-4 py-2 bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    </div>

    <!-- Loading Overlay for Long Operations -->
    <div
      v-if="categorizing || transactionsStore.loading"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
    >
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-2xl p-8 max-w-sm w-full mx-4 text-center">
        <div class="relative w-16 h-16 mx-auto mb-4">
          <svg class="animate-spin w-16 h-16 text-purple-600" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
          </svg>
        </div>
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">
          {{ categorizing ? 'Auto-Categorizing...' : 'Loading Transactions...' }}
        </h3>
        <p class="text-sm text-gray-500 dark:text-gray-400">
          {{ categorizing
            ? 'Analyzing transactions and applying category rules. This may take a moment for large datasets.'
            : 'Fetching your transactions. This may take a moment for large datasets.'
          }}
        </p>
      </div>
    </div>

    <!-- Floating Action Button for Batch Categorize (visible when scrolling) -->
    <div class="fixed bottom-6 right-6 flex flex-col gap-3 z-40">
      <button
        v-if="uncategorizedCount > 0"
        @click="autoCategorize"
        :disabled="categorizing"
        class="w-14 h-14 bg-purple-600 text-white rounded-full shadow-lg hover:bg-purple-700 transition-all disabled:opacity-50 flex items-center justify-center group"
        title="Auto-Categorize"
      >
        <svg v-if="categorizing" class="animate-spin h-6 w-6" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
        </svg>
        <svg v-else class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z" />
        </svg>
        <span class="absolute -top-2 -right-2 bg-red-500 text-white text-xs rounded-full w-6 h-6 flex items-center justify-center">
          {{ uncategorizedCount > 99 ? '99+' : uncategorizedCount }}
        </span>
      </button>
      <button
        @click="showBatchModal = true"
        class="w-14 h-14 bg-indigo-600 text-white rounded-full shadow-lg hover:bg-indigo-700 transition-all flex items-center justify-center"
        title="Batch Categorize"
      >
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
        </svg>
      </button>
    </div>
  </div>
</template>
