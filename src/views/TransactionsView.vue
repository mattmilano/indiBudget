<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { useTransactionsStore, useAccountsStore, useCategoriesStore } from '../stores';
import type { CreateTransactionRequest, TransactionType, AutoCategorizeResult } from '../types';
import { format } from 'date-fns';
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
    result = result.filter(t => t.category_id === filterCategoryId.value);
  }

  if (filterType.value) {
    result = result.filter(t => t.transaction_type === filterType.value);
  }

  return result;
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
  return transactionsStore.transactions.filter(t => !t.category_id).length;
});

onMounted(async () => {
  await Promise.all([
    accountsStore.fetchAccounts(),
    categoriesStore.fetchCategories(),
    transactionsStore.fetchTransactions(),
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
          @click="showAddModal = true"
          class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          Add Transaction
        </button>
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
        No transactions found. Add your first transaction or import a statement!
      </div>
      <div v-else class="divide-y divide-gray-200 dark:divide-gray-700">
        <div
          v-for="tx in filteredTransactions"
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
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center gap-3">
          <div class="w-10 h-10 bg-green-100 dark:bg-green-900 rounded-full flex items-center justify-center">
            <svg class="w-5 h-5 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
            </svg>
          </div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Categorization Complete</h3>
        </div>
        <div class="p-4">
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
        <div class="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end">
          <button
            @click="showCategorizeResultModal = false"
            class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
