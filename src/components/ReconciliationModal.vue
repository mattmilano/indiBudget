<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';
import { useTransactionsStore, useCategoriesStore } from '../stores';
import type { Account, Transaction, TransactionStatus } from '../types';
import * as api from '../services/api';

interface Props {
  show: boolean;
  account: Account | null;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'complete'): void;
}>();

const transactionsStore = useTransactionsStore();
const categoriesStore = useCategoriesStore();

const statementBalance = ref('');
const statementDate = ref('');
const saving = ref(false);
const localStatuses = ref<Record<string, TransactionStatus>>({});

// Initialize statement date to today
onMounted(() => {
  const today = new Date();
  statementDate.value = today.toISOString().split('T')[0];
});

const formatCurrency = (value: number | string) => {
  const num = typeof value === 'string' ? parseFloat(value) : value;
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(num || 0);
};

// Get unreconciled transactions for this account
const accountTransactions = computed(() => {
  if (!props.account) return [];
  return transactionsStore.sortedTransactions
    .filter(t => t.account_id === props.account?.id && t.status !== 'void')
    .filter(t => !statementDate.value || t.date <= statementDate.value);
});

const unreconciledTransactions = computed(() => {
  return accountTransactions.value.filter(t => {
    const status = localStatuses.value[t.id] || t.status;
    return status !== 'reconciled';
  });
});

const reconciledTransactions = computed(() => {
  return accountTransactions.value.filter(t => {
    const status = localStatuses.value[t.id] || t.status;
    return status === 'reconciled';
  });
});

// Calculate balances
const clearedBalance = computed(() => {
  let balance = 0;
  for (const tx of accountTransactions.value) {
    const status = localStatuses.value[tx.id] || tx.status;
    if (status === 'cleared' || status === 'reconciled') {
      const amount = parseFloat(tx.amount) || 0;
      if (tx.transaction_type === 'income') {
        balance += amount;
      } else if (tx.transaction_type === 'expense') {
        balance -= amount;
      }
    }
  }
  return balance;
});

const targetStatementBalance = computed(() => {
  return parseFloat(statementBalance.value) || 0;
});

const difference = computed(() => {
  return targetStatementBalance.value - clearedBalance.value;
});

const isBalanced = computed(() => {
  return Math.abs(difference.value) < 0.01;
});

function getTransactionStatus(tx: Transaction): TransactionStatus {
  return localStatuses.value[tx.id] || tx.status;
}

function toggleCleared(tx: Transaction) {
  const currentStatus = getTransactionStatus(tx);
  if (currentStatus === 'cleared') {
    localStatuses.value[tx.id] = 'pending';
  } else if (currentStatus === 'pending') {
    localStatuses.value[tx.id] = 'cleared';
  }
}

function markAllCleared() {
  unreconciledTransactions.value.forEach(tx => {
    if (getTransactionStatus(tx) === 'pending') {
      localStatuses.value[tx.id] = 'cleared';
    }
  });
}

async function handleReconcile() {
  if (!isBalanced.value) {
    if (!confirm(`The difference is ${formatCurrency(difference.value)}. Continue anyway?`)) {
      return;
    }
  }

  saving.value = true;
  try {
    // Update all cleared transactions to reconciled
    for (const tx of accountTransactions.value) {
      const status = localStatuses.value[tx.id] || tx.status;
      if (status === 'cleared') {
        await api.updateTransaction({
          id: tx.id,
          status: 'reconciled',
        });
      } else if (localStatuses.value[tx.id] && localStatuses.value[tx.id] !== tx.status) {
        // Also update any status changes we made
        await api.updateTransaction({
          id: tx.id,
          status: localStatuses.value[tx.id],
        });
      }
    }

    await transactionsStore.fetchTransactions({});
    emit('complete');
  } catch (e) {
    console.error('Failed to reconcile:', e);
    alert('Failed to reconcile transactions.');
  } finally {
    saving.value = false;
  }
}

function handleClose() {
  localStatuses.value = {};
  emit('close');
}

// Reset local statuses when modal opens
watch(() => props.show, (newVal) => {
  if (newVal) {
    localStatuses.value = {};
    statementBalance.value = props.account?.balance || '';
  }
});
</script>

<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition ease-out duration-200"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="transition ease-in duration-150"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="show && account"
        class="fixed inset-0 z-50 overflow-y-auto"
        @click.self="handleClose"
      >
        <div class="flex min-h-full items-center justify-center p-4">
          <!-- Backdrop -->
          <div class="fixed inset-0 bg-black/50 transition-opacity" @click="handleClose" />

          <!-- Modal -->
          <div class="relative bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-4xl w-full mx-4 overflow-hidden max-h-[90vh] flex flex-col">
            <!-- Header -->
            <div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex-shrink-0">
              <div class="flex items-center justify-between">
                <div>
                  <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
                    Reconcile Account
                  </h3>
                  <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
                    {{ account.name }}
                  </p>
                </div>
                <button
                  @click="handleClose"
                  class="text-gray-400 hover:text-gray-500 dark:hover:text-gray-300"
                >
                  <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
            </div>

            <!-- Statement Info -->
            <div class="px-6 py-4 bg-gray-50 dark:bg-gray-700/50 border-b border-gray-200 dark:border-gray-700 flex-shrink-0">
              <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Statement Date
                  </label>
                  <input
                    v-model="statementDate"
                    type="date"
                    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                  />
                </div>
                <div>
                  <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Statement Ending Balance
                  </label>
                  <div class="relative">
                    <span class="absolute left-3 top-1/2 -translate-y-1/2 text-gray-500">$</span>
                    <input
                      v-model="statementBalance"
                      type="number"
                      step="0.01"
                      class="w-full pl-7 pr-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                      placeholder="0.00"
                    />
                  </div>
                </div>
              </div>

              <!-- Balance Summary -->
              <div class="mt-4 grid grid-cols-3 gap-4 text-center">
                <div class="p-3 bg-white dark:bg-gray-800 rounded-lg">
                  <p class="text-xs text-gray-500 dark:text-gray-400">Cleared Balance</p>
                  <p class="text-lg font-semibold text-gray-900 dark:text-white">
                    {{ formatCurrency(clearedBalance) }}
                  </p>
                </div>
                <div class="p-3 bg-white dark:bg-gray-800 rounded-lg">
                  <p class="text-xs text-gray-500 dark:text-gray-400">Statement Balance</p>
                  <p class="text-lg font-semibold text-blue-600 dark:text-blue-400">
                    {{ formatCurrency(targetStatementBalance) }}
                  </p>
                </div>
                <div :class="[
                  'p-3 rounded-lg',
                  isBalanced
                    ? 'bg-green-100 dark:bg-green-900/30'
                    : 'bg-yellow-100 dark:bg-yellow-900/30'
                ]">
                  <p class="text-xs text-gray-500 dark:text-gray-400">Difference</p>
                  <p :class="[
                    'text-lg font-semibold',
                    isBalanced ? 'text-green-600 dark:text-green-400' : 'text-yellow-600 dark:text-yellow-400'
                  ]">
                    {{ formatCurrency(difference) }}
                  </p>
                </div>
              </div>
            </div>

            <!-- Transactions List -->
            <div class="flex-1 overflow-y-auto px-6 py-4">
              <div class="flex items-center justify-between mb-3">
                <h4 class="font-medium text-gray-900 dark:text-white">
                  Unreconciled Transactions ({{ unreconciledTransactions.length }})
                </h4>
                <button
                  @click="markAllCleared"
                  class="text-sm text-blue-600 dark:text-blue-400 hover:underline"
                >
                  Mark All Cleared
                </button>
              </div>

              <div v-if="unreconciledTransactions.length === 0" class="text-center py-8 text-gray-500">
                No unreconciled transactions for this period.
              </div>

              <div v-else class="space-y-2">
                <div
                  v-for="tx in unreconciledTransactions"
                  :key="tx.id"
                  @click="toggleCleared(tx)"
                  :class="[
                    'p-3 rounded-lg border cursor-pointer transition-all',
                    getTransactionStatus(tx) === 'cleared'
                      ? 'border-green-300 bg-green-50 dark:bg-green-900/20 dark:border-green-700'
                      : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 hover:bg-gray-50 dark:hover:bg-gray-700'
                  ]"
                >
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-3">
                      <div :class="[
                        'w-5 h-5 rounded border-2 flex items-center justify-center',
                        getTransactionStatus(tx) === 'cleared'
                          ? 'border-green-500 bg-green-500'
                          : 'border-gray-300 dark:border-gray-600'
                      ]">
                        <svg v-if="getTransactionStatus(tx) === 'cleared'" class="w-3 h-3 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
                        </svg>
                      </div>
                      <div>
                        <p class="font-medium text-gray-900 dark:text-white text-sm">
                          {{ tx.description }}
                        </p>
                        <p class="text-xs text-gray-500 dark:text-gray-400">
                          {{ tx.date }}
                          <span v-if="tx.category_id" class="ml-1">
                            · {{ categoriesStore.getCategoryById(tx.category_id)?.name }}
                          </span>
                        </p>
                      </div>
                    </div>
                    <p :class="[
                      'font-semibold',
                      tx.transaction_type === 'income' ? 'text-green-600' : 'text-red-600'
                    ]">
                      {{ tx.transaction_type === 'income' ? '+' : '-' }}{{ formatCurrency(tx.amount) }}
                    </p>
                  </div>
                </div>
              </div>

              <!-- Already Reconciled Section -->
              <div v-if="reconciledTransactions.length > 0" class="mt-6">
                <h4 class="font-medium text-gray-500 dark:text-gray-400 mb-3">
                  Previously Reconciled ({{ reconciledTransactions.length }})
                </h4>
                <div class="space-y-1 opacity-60">
                  <div
                    v-for="tx in reconciledTransactions.slice(0, 5)"
                    :key="tx.id"
                    class="p-2 rounded bg-gray-100 dark:bg-gray-700/50 text-sm flex justify-between"
                  >
                    <span class="text-gray-600 dark:text-gray-400">{{ tx.description }}</span>
                    <span :class="tx.transaction_type === 'income' ? 'text-green-600' : 'text-red-600'">
                      {{ tx.transaction_type === 'income' ? '+' : '-' }}{{ formatCurrency(tx.amount) }}
                    </span>
                  </div>
                  <p v-if="reconciledTransactions.length > 5" class="text-xs text-gray-400 text-center">
                    and {{ reconciledTransactions.length - 5 }} more...
                  </p>
                </div>
              </div>
            </div>

            <!-- Footer -->
            <div class="px-6 py-4 bg-gray-50 dark:bg-gray-700/50 border-t border-gray-200 dark:border-gray-700 flex-shrink-0">
              <div class="flex justify-between items-center">
                <p class="text-sm text-gray-500 dark:text-gray-400">
                  Click transactions to mark them as cleared
                </p>
                <div class="flex gap-3">
                  <button
                    type="button"
                    @click="handleClose"
                    class="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-600 transition-colors"
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    @click="handleReconcile"
                    :disabled="saving"
                    :class="[
                      'px-4 py-2 text-sm font-medium text-white rounded-lg focus:outline-none focus:ring-2 focus:ring-offset-2 transition-colors flex items-center gap-2',
                      isBalanced
                        ? 'bg-green-600 hover:bg-green-700 focus:ring-green-500'
                        : 'bg-yellow-600 hover:bg-yellow-700 focus:ring-yellow-500'
                    ]"
                  >
                    <svg v-if="saving" class="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
                      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                    </svg>
                    {{ saving ? 'Reconciling...' : isBalanced ? 'Finish Reconciliation' : 'Reconcile Anyway' }}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
