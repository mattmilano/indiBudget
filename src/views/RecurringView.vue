<script setup lang="ts">
import { ref, onMounted } from 'vue';
import * as api from '../services/api';
import { useAccountsStore, useCategoriesStore } from '../stores';
import type { RecurringTransaction, DetectedRecurring, CreateRecurringRequest } from '../types';

const accountsStore = useAccountsStore();
const categoriesStore = useCategoriesStore();

const recurring = ref<RecurringTransaction[]>([]);
const detectedPatterns = ref<DetectedRecurring[]>([]);
const loading = ref(false);
const detectingPatterns = ref(false);
const showAddModal = ref(false);
const showDetectedModal = ref(false);

const newRecurring = ref<CreateRecurringRequest>({
  account_id: '',
  transaction_type: 'expense',
  amount: '',
  description: '',
  frequency: 'monthly',
  start_date: new Date().toISOString().split('T')[0],
});

onMounted(async () => {
  loading.value = true;
  await Promise.all([
    accountsStore.fetchAccounts(),
    categoriesStore.fetchCategories(),
    fetchRecurring(),
  ]);
  loading.value = false;
});

async function fetchRecurring() {
  try {
    recurring.value = await api.getRecurring();
  } catch (e) {
    console.error('Failed to fetch recurring:', e);
  }
}

async function detectPatterns() {
  detectingPatterns.value = true;
  try {
    detectedPatterns.value = await api.detectRecurringPatterns();
    if (detectedPatterns.value.length > 0) {
      showDetectedModal.value = true;
    } else {
      alert('No recurring patterns detected in your transactions.');
    }
  } catch (e) {
    console.error('Failed to detect patterns:', e);
    alert('Failed to analyze transactions for patterns.');
  } finally {
    detectingPatterns.value = false;
  }
}

async function confirmDetected(detected: DetectedRecurring) {
  try {
    await api.createRecurringFromDetected(detected);
    // Remove from detected list
    detectedPatterns.value = detectedPatterns.value.filter(d => d !== detected);
    // Refresh recurring list
    await fetchRecurring();
  } catch (e) {
    console.error('Failed to create recurring:', e);
    alert('Failed to create recurring transaction.');
  }
}

function dismissDetected(detected: DetectedRecurring) {
  detectedPatterns.value = detectedPatterns.value.filter(d => d !== detected);
}

async function createRecurring() {
  if (!newRecurring.value.account_id || !newRecurring.value.amount || !newRecurring.value.description) {
    return;
  }
  try {
    await api.createRecurring(newRecurring.value);
    showAddModal.value = false;
    newRecurring.value = {
      account_id: accountsStore.accounts[0]?.id || '',
      transaction_type: 'expense',
      amount: '',
      description: '',
      frequency: 'monthly',
      start_date: new Date().toISOString().split('T')[0],
    };
    await fetchRecurring();
  } catch (e) {
    console.error('Failed to create recurring:', e);
  }
}

function formatAmount(amount: string): string {
  const num = parseFloat(amount);
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(Math.abs(num));
}

function formatFrequency(freq: string): string {
  const map: Record<string, string> = {
    daily: 'Daily',
    weekly: 'Weekly',
    biweekly: 'Every 2 weeks',
    monthly: 'Monthly',
    quarterly: 'Quarterly',
    yearly: 'Yearly',
  };
  return map[freq] || freq;
}

function formatDate(date: string): string {
  return new Date(date).toLocaleDateString();
}

function getAccountName(id: string): string {
  return accountsStore.accountsById[id]?.name || 'Unknown';
}

function getCategoryName(id?: string): string {
  if (!id) return 'Uncategorized';
  const cat = categoriesStore.categoriesById[id];
  return cat?.name || 'Unknown';
}

function getConfidenceColor(confidence: number): string {
  if (confidence >= 0.8) return 'text-green-600 dark:text-green-400';
  if (confidence >= 0.6) return 'text-yellow-600 dark:text-yellow-400';
  return 'text-orange-600 dark:text-orange-400';
}

function getConfidenceLabel(confidence: number): string {
  if (confidence >= 0.8) return 'High';
  if (confidence >= 0.6) return 'Medium';
  return 'Low';
}
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Recurring Transactions</h1>
      <div class="flex gap-3">
        <button
          @click="detectPatterns"
          :disabled="detectingPatterns"
          class="px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 transition-colors disabled:opacity-50 flex items-center gap-2"
        >
          <svg v-if="detectingPatterns" class="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
          </svg>
          <svg v-else class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
          </svg>
          {{ detectingPatterns ? 'Analyzing...' : 'Detect Patterns' }}
        </button>
        <button
          @click="showAddModal = true"
          class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors flex items-center gap-2"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
          </svg>
          Add Recurring
        </button>
      </div>
    </div>

    <!-- Loading -->
    <div v-if="loading" class="flex justify-center py-12">
      <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
    </div>

    <!-- Recurring List -->
    <div v-else-if="recurring.length > 0" class="bg-white dark:bg-gray-800 rounded-lg shadow overflow-hidden">
      <table class="w-full">
        <thead class="bg-gray-50 dark:bg-gray-700">
          <tr>
            <th class="px-4 py-3 text-left text-sm font-medium text-gray-700 dark:text-gray-300">Description</th>
            <th class="px-4 py-3 text-left text-sm font-medium text-gray-700 dark:text-gray-300">Amount</th>
            <th class="px-4 py-3 text-left text-sm font-medium text-gray-700 dark:text-gray-300">Frequency</th>
            <th class="px-4 py-3 text-left text-sm font-medium text-gray-700 dark:text-gray-300">Next Due</th>
            <th class="px-4 py-3 text-left text-sm font-medium text-gray-700 dark:text-gray-300">Account</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-gray-200 dark:divide-gray-700">
          <tr v-for="item in recurring" :key="item.id" class="hover:bg-gray-50 dark:hover:bg-gray-700/50">
            <td class="px-4 py-3">
              <div class="font-medium text-gray-900 dark:text-white">{{ item.description }}</div>
              <div v-if="item.category_id" class="text-sm text-gray-500 dark:text-gray-400">
                {{ getCategoryName(item.category_id) }}
              </div>
            </td>
            <td class="px-4 py-3">
              <span :class="item.transaction_type === 'income' ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400'">
                {{ item.transaction_type === 'income' ? '+' : '-' }}{{ formatAmount(item.amount) }}
              </span>
            </td>
            <td class="px-4 py-3 text-gray-700 dark:text-gray-300">
              {{ formatFrequency(item.frequency) }}
            </td>
            <td class="px-4 py-3 text-gray-700 dark:text-gray-300">
              {{ formatDate(item.next_occurrence) }}
            </td>
            <td class="px-4 py-3 text-gray-700 dark:text-gray-300">
              {{ getAccountName(item.account_id) }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Empty State -->
    <div v-else class="bg-white dark:bg-gray-800 rounded-lg shadow p-12 text-center">
      <svg class="mx-auto h-12 w-12 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
      </svg>
      <h3 class="mt-4 text-lg font-medium text-gray-900 dark:text-white">No recurring transactions</h3>
      <p class="mt-2 text-gray-500 dark:text-gray-400">
        Click "Detect Patterns" to automatically find recurring transactions from your history,<br>
        or add them manually.
      </p>
    </div>

    <!-- Detected Patterns Modal -->
    <div v-if="showDetectedModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-3xl w-full max-h-[80vh] flex flex-col">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">
            Detected Recurring Patterns ({{ detectedPatterns.length }})
          </h2>
          <button @click="showDetectedModal = false" class="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <div class="p-4 overflow-y-auto flex-1">
          <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
            We analyzed your transactions and found these potential recurring patterns. Review and confirm the ones you want to track.
          </p>
          <div class="space-y-4">
            <div
              v-for="(detected, index) in detectedPatterns"
              :key="index"
              class="border border-gray-200 dark:border-gray-700 rounded-lg p-4"
            >
              <div class="flex justify-between items-start">
                <div class="flex-1">
                  <div class="flex items-center gap-3">
                    <h3 class="font-medium text-gray-900 dark:text-white">{{ detected.description }}</h3>
                    <span :class="['text-xs font-medium', getConfidenceColor(detected.confidence)]">
                      {{ getConfidenceLabel(detected.confidence) }} confidence
                    </span>
                  </div>
                  <div class="mt-2 grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
                    <div class="text-gray-600 dark:text-gray-400">
                      Amount: <span :class="detected.transaction_type === 'income' ? 'text-green-600' : 'text-red-600'">
                        {{ formatAmount(detected.average_amount) }}
                      </span>
                      <span v-if="!detected.amount_is_consistent" class="text-xs text-gray-500">(varies)</span>
                    </div>
                    <div class="text-gray-600 dark:text-gray-400">
                      Frequency: {{ formatFrequency(detected.frequency) }}
                    </div>
                    <div class="text-gray-600 dark:text-gray-400">
                      Occurrences: {{ detected.occurrence_count }}
                    </div>
                    <div v-if="detected.typical_day_of_month" class="text-gray-600 dark:text-gray-400">
                      Typical day: {{ detected.typical_day_of_month }}
                    </div>
                  </div>
                </div>
                <div class="flex gap-2 ml-4">
                  <button
                    @click="confirmDetected(detected)"
                    class="px-3 py-1.5 bg-green-600 text-white text-sm rounded-lg hover:bg-green-700 transition-colors"
                  >
                    Add
                  </button>
                  <button
                    @click="dismissDetected(detected)"
                    class="px-3 py-1.5 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 text-sm rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
                  >
                    Dismiss
                  </button>
                </div>
              </div>
            </div>
          </div>
          <div v-if="detectedPatterns.length === 0" class="text-center py-8 text-gray-500 dark:text-gray-400">
            All patterns have been reviewed!
          </div>
        </div>
        <div class="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end">
          <button
            @click="showDetectedModal = false"
            class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    </div>

    <!-- Add Recurring Modal -->
    <div v-if="showAddModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Add Recurring Transaction</h2>
        </div>
        <div class="p-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Description</label>
            <input
              v-model="newRecurring.description"
              type="text"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              placeholder="e.g., Netflix subscription"
            />
          </div>
          <div class="grid grid-cols-2 gap-4">
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Amount</label>
              <input
                v-model="newRecurring.amount"
                type="number"
                step="0.01"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                placeholder="0.00"
              />
            </div>
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Type</label>
              <select
                v-model="newRecurring.transaction_type"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              >
                <option value="expense">Expense</option>
                <option value="income">Income</option>
              </select>
            </div>
          </div>
          <div class="grid grid-cols-2 gap-4">
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Frequency</label>
              <select
                v-model="newRecurring.frequency"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              >
                <option value="weekly">Weekly</option>
                <option value="biweekly">Every 2 weeks</option>
                <option value="monthly">Monthly</option>
                <option value="quarterly">Quarterly</option>
                <option value="yearly">Yearly</option>
              </select>
            </div>
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Start Date</label>
              <input
                v-model="newRecurring.start_date"
                type="date"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
            </div>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Account</label>
            <select
              v-model="newRecurring.account_id"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            >
              <option value="">Select account...</option>
              <option v-for="account in accountsStore.accounts" :key="account.id" :value="account.id">
                {{ account.name }}
              </option>
            </select>
          </div>
        </div>
        <div class="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-3">
          <button
            @click="showAddModal = false"
            class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
          >
            Cancel
          </button>
          <button
            @click="createRecurring"
            class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
          >
            Add
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
