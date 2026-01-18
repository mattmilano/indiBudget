<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { useTransactionsStore, useCategoriesStore } from '../stores';

const transactionsStore = useTransactionsStore();
const categoriesStore = useCategoriesStore();

const searchQuery = ref('');
const sortBy = ref<'name' | 'count' | 'total'>('total');
const sortOrder = ref<'asc' | 'desc'>('desc');

// Aggregate payee data from transactions
const payeesData = computed(() => {
  const payeeMap = new Map<string, {
    name: string;
    transactionCount: number;
    totalSpent: number;
    lastTransaction: string;
    categories: Set<string>;
    transactions: typeof transactionsStore.transactions;
  }>();

  for (const tx of transactionsStore.transactions) {
    const payeeName = tx.payee || tx.description;
    if (!payeeName) continue;

    const existing = payeeMap.get(payeeName.toLowerCase()) || {
      name: payeeName,
      transactionCount: 0,
      totalSpent: 0,
      lastTransaction: tx.date,
      categories: new Set<string>(),
      transactions: [],
    };

    existing.transactionCount++;
    if (tx.transaction_type === 'expense') {
      existing.totalSpent += parseFloat(tx.amount) || 0;
    }
    if (tx.date > existing.lastTransaction) {
      existing.lastTransaction = tx.date;
    }
    if (tx.category_id) {
      existing.categories.add(tx.category_id);
    }
    existing.transactions.push(tx);

    payeeMap.set(payeeName.toLowerCase(), existing);
  }

  return Array.from(payeeMap.values());
});

// Filter and sort payees
const filteredPayees = computed(() => {
  let result = payeesData.value;

  // Apply search filter
  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase();
    result = result.filter(p => p.name.toLowerCase().includes(query));
  }

  // Sort
  result.sort((a, b) => {
    let comparison = 0;
    switch (sortBy.value) {
      case 'name':
        comparison = a.name.localeCompare(b.name);
        break;
      case 'count':
        comparison = a.transactionCount - b.transactionCount;
        break;
      case 'total':
        comparison = a.totalSpent - b.totalSpent;
        break;
    }
    return sortOrder.value === 'desc' ? -comparison : comparison;
  });

  return result;
});

// Summary stats
const summaryStats = computed(() => ({
  totalPayees: payeesData.value.length,
  totalSpent: payeesData.value.reduce((sum, p) => sum + p.totalSpent, 0),
  avgPerPayee: payeesData.value.length > 0
    ? payeesData.value.reduce((sum, p) => sum + p.totalSpent, 0) / payeesData.value.length
    : 0,
}));

const formatCurrency = (value: number) => {
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(value);
};

const formatDate = (date: string) => {
  return new Date(date).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
};

const getCategoryNames = (categoryIds: Set<string>) => {
  return Array.from(categoryIds)
    .map(id => categoriesStore.getCategoryById(id)?.name)
    .filter(Boolean)
    .slice(0, 3);
};

const toggleSort = (field: 'name' | 'count' | 'total') => {
  if (sortBy.value === field) {
    sortOrder.value = sortOrder.value === 'asc' ? 'desc' : 'asc';
  } else {
    sortBy.value = field;
    sortOrder.value = 'desc';
  }
};

onMounted(async () => {
  await Promise.all([
    transactionsStore.fetchTransactions({}),
    categoriesStore.fetchCategories(),
  ]);
});
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Payees & Merchants</h1>
    </div>

    <!-- Summary Cards -->
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
      <div class="bg-gradient-to-r from-blue-600 to-blue-700 rounded-lg shadow-lg p-6 text-white">
        <h3 class="text-sm font-medium opacity-90">Total Payees</h3>
        <p class="text-3xl font-bold mt-1">{{ summaryStats.totalPayees }}</p>
      </div>
      <div class="bg-gradient-to-r from-red-600 to-red-700 rounded-lg shadow-lg p-6 text-white">
        <h3 class="text-sm font-medium opacity-90">Total Spending</h3>
        <p class="text-3xl font-bold mt-1">{{ formatCurrency(summaryStats.totalSpent) }}</p>
      </div>
      <div class="bg-gradient-to-r from-purple-600 to-purple-700 rounded-lg shadow-lg p-6 text-white">
        <h3 class="text-sm font-medium opacity-90">Avg per Payee</h3>
        <p class="text-3xl font-bold mt-1">{{ formatCurrency(summaryStats.avgPerPayee) }}</p>
      </div>
    </div>

    <!-- Search and Sort -->
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4 mb-6">
      <div class="flex flex-col md:flex-row gap-4">
        <div class="flex-1">
          <input
            v-model="searchQuery"
            type="text"
            placeholder="Search payees..."
            class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
          />
        </div>
        <div class="flex gap-2">
          <button
            @click="toggleSort('name')"
            :class="[
              'px-4 py-2 rounded-lg font-medium transition-colors flex items-center gap-1',
              sortBy === 'name'
                ? 'bg-blue-600 text-white'
                : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300'
            ]"
          >
            Name
            <svg v-if="sortBy === 'name'" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" :d="sortOrder === 'asc' ? 'M5 15l7-7 7 7' : 'M19 9l-7 7-7-7'" />
            </svg>
          </button>
          <button
            @click="toggleSort('count')"
            :class="[
              'px-4 py-2 rounded-lg font-medium transition-colors flex items-center gap-1',
              sortBy === 'count'
                ? 'bg-blue-600 text-white'
                : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300'
            ]"
          >
            Count
            <svg v-if="sortBy === 'count'" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" :d="sortOrder === 'asc' ? 'M5 15l7-7 7 7' : 'M19 9l-7 7-7-7'" />
            </svg>
          </button>
          <button
            @click="toggleSort('total')"
            :class="[
              'px-4 py-2 rounded-lg font-medium transition-colors flex items-center gap-1',
              sortBy === 'total'
                ? 'bg-blue-600 text-white'
                : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300'
            ]"
          >
            Amount
            <svg v-if="sortBy === 'total'" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" :d="sortOrder === 'asc' ? 'M5 15l7-7 7 7' : 'M19 9l-7 7-7-7'" />
            </svg>
          </button>
        </div>
      </div>
    </div>

    <!-- Payees List -->
    <div v-if="filteredPayees.length > 0" class="bg-white dark:bg-gray-800 rounded-lg shadow overflow-hidden">
      <div class="divide-y divide-gray-200 dark:divide-gray-700">
        <div
          v-for="payee in filteredPayees"
          :key="payee.name"
          class="p-4 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
        >
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-4">
              <div class="w-12 h-12 bg-gray-100 dark:bg-gray-700 rounded-full flex items-center justify-center">
                <span class="text-lg font-bold text-gray-600 dark:text-gray-400">
                  {{ payee.name.charAt(0).toUpperCase() }}
                </span>
              </div>
              <div>
                <h3 class="font-semibold text-gray-900 dark:text-white">{{ payee.name }}</h3>
                <div class="flex items-center gap-3 mt-1">
                  <span class="text-sm text-gray-500 dark:text-gray-400">
                    {{ payee.transactionCount }} transaction{{ payee.transactionCount !== 1 ? 's' : '' }}
                  </span>
                  <span class="text-sm text-gray-400">|</span>
                  <span class="text-sm text-gray-500 dark:text-gray-400">
                    Last: {{ formatDate(payee.lastTransaction) }}
                  </span>
                </div>
                <div v-if="getCategoryNames(payee.categories).length > 0" class="flex gap-1 mt-2">
                  <span
                    v-for="catName in getCategoryNames(payee.categories)"
                    :key="catName"
                    class="px-2 py-0.5 text-xs font-medium bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 rounded"
                  >
                    {{ catName }}
                  </span>
                  <span
                    v-if="payee.categories.size > 3"
                    class="px-2 py-0.5 text-xs font-medium bg-gray-100 dark:bg-gray-700 text-gray-500 rounded"
                  >
                    +{{ payee.categories.size - 3 }} more
                  </span>
                </div>
              </div>
            </div>
            <div class="text-right">
              <p class="text-xl font-bold text-red-600">{{ formatCurrency(payee.totalSpent) }}</p>
              <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
                {{ formatCurrency(payee.totalSpent / payee.transactionCount) }} avg
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Empty State -->
    <div v-else class="bg-white dark:bg-gray-800 rounded-lg shadow p-12 text-center">
      <svg class="w-16 h-16 mx-auto text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
      </svg>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mt-4">
        {{ searchQuery ? 'No matching payees found' : 'No payees yet' }}
      </h3>
      <p class="text-gray-500 dark:text-gray-400 mt-2">
        {{ searchQuery ? 'Try adjusting your search.' : 'Import transactions or add payee information to see them here.' }}
      </p>
    </div>
  </div>
</template>
