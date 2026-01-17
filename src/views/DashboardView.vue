<script setup lang="ts">
import { onMounted, computed, ref } from 'vue';
import { useAccountsStore, useTransactionsStore, useBudgetsStore, useCalendarStore, useCategoriesStore } from '../stores';
import { format, startOfMonth, endOfMonth } from 'date-fns';
import { useRouter } from 'vue-router';

const router = useRouter();
const accountsStore = useAccountsStore();
const transactionsStore = useTransactionsStore();
const budgetsStore = useBudgetsStore();
const calendarStore = useCalendarStore();
const categoriesStore = useCategoriesStore();

const loading = ref(true);

const today = new Date();
const monthStart = format(startOfMonth(today), 'yyyy-MM-dd');
const monthEnd = format(endOfMonth(today), 'yyyy-MM-dd');

onMounted(async () => {
  loading.value = true;
  await Promise.all([
    accountsStore.fetchAccounts(),
    transactionsStore.fetchTransactions({ start_date: monthStart, end_date: monthEnd }),
    budgetsStore.fetchBudgetStatus(),
    calendarStore.fetchUpcomingRecurring(30),
    categoriesStore.fetchCategories(),
  ]);
  loading.value = false;
});

const formatCurrency = (value: number | string) => {
  const num = typeof value === 'string' ? parseFloat(value) : value;
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(num || 0);
};

const monthlyIncome = computed(() => {
  return transactionsStore.transactions
    .filter(t => t.transaction_type === 'income')
    .reduce((sum, t) => sum + (parseFloat(t.amount) || 0), 0);
});

const monthlyExpenses = computed(() => {
  return transactionsStore.transactions
    .filter(t => t.transaction_type === 'expense')
    .reduce((sum, t) => sum + (parseFloat(t.amount) || 0), 0);
});

const netSavings = computed(() => monthlyIncome.value - monthlyExpenses.value);

const savingsRate = computed(() => {
  if (monthlyIncome.value === 0) return 0;
  return ((monthlyIncome.value - monthlyExpenses.value) / monthlyIncome.value) * 100;
});

// Top spending categories
const topCategories = computed(() => {
  const categorySpending: Record<string, { name: string; color: string; amount: number }> = {};

  transactionsStore.transactions
    .filter(t => t.transaction_type === 'expense' && t.category_id)
    .forEach(t => {
      const category = categoriesStore.categoriesById[t.category_id!];
      if (category) {
        if (!categorySpending[category.id]) {
          categorySpending[category.id] = { name: category.name, color: category.color, amount: 0 };
        }
        categorySpending[category.id].amount += parseFloat(t.amount) || 0;
      }
    });

  return Object.values(categorySpending)
    .sort((a, b) => b.amount - a.amount)
    .slice(0, 5);
});

// Uncategorized transaction count
const uncategorizedCount = computed(() => {
  return transactionsStore.transactions.filter(t => !t.category_id).length;
});

// Check if user is new (no data)
const isNewUser = computed(() => {
  return accountsStore.accounts.length === 0 && transactionsStore.transactions.length === 0;
});

// Quick navigation helpers
function navigateTo(path: string) {
  router.push(path);
}
</script>

<template>
  <div class="p-6 space-y-6">
    <div class="flex justify-between items-center">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Dashboard</h1>
      <p class="text-sm text-gray-500 dark:text-gray-400">{{ format(today, 'MMMM yyyy') }}</p>
    </div>

    <!-- Loading State -->
    <div v-if="loading" class="flex justify-center py-12">
      <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
    </div>

    <!-- New User Welcome -->
    <div v-else-if="isNewUser" class="bg-gradient-to-br from-indigo-500 to-purple-600 rounded-2xl shadow-lg p-8 text-white">
      <div class="max-w-2xl">
        <h2 class="text-3xl font-bold mb-4">Welcome to indiBudget!</h2>
        <p class="text-indigo-100 mb-6">
          Get started by setting up your accounts and importing your first bank statement.
          We'll help you track your spending, set budgets, and reach your financial goals.
        </p>
        <div class="flex flex-wrap gap-4">
          <button
            @click="navigateTo('/accounts')"
            class="px-6 py-3 bg-white text-indigo-600 font-semibold rounded-lg hover:bg-indigo-50 transition-colors"
          >
            Add Your First Account
          </button>
          <button
            @click="navigateTo('/import')"
            class="px-6 py-3 bg-indigo-400 text-white font-semibold rounded-lg hover:bg-indigo-300 transition-colors"
          >
            Import Transactions
          </button>
        </div>
      </div>
    </div>

    <template v-else>
      <!-- Summary Cards -->
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Total Balance</h3>
          <p class="text-2xl font-bold text-gray-900 dark:text-white mt-2">
            {{ formatCurrency(accountsStore.totalBalance) }}
          </p>
          <p class="text-sm text-gray-500 mt-1">{{ accountsStore.activeAccounts.length }} accounts</p>
        </div>

        <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Monthly Income</h3>
          <p class="text-2xl font-bold text-green-600 dark:text-green-400 mt-2">
            {{ formatCurrency(monthlyIncome) }}
          </p>
          <p class="text-sm text-gray-500 mt-1">This month</p>
        </div>

        <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Monthly Expenses</h3>
          <p class="text-2xl font-bold text-red-600 dark:text-red-400 mt-2">
            {{ formatCurrency(monthlyExpenses) }}
          </p>
          <p class="text-sm text-gray-500 mt-1">This month</p>
        </div>

        <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Net Savings</h3>
          <p :class="['text-2xl font-bold mt-2', netSavings >= 0 ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400']">
            {{ netSavings >= 0 ? '+' : '' }}{{ formatCurrency(netSavings) }}
          </p>
          <p class="text-sm mt-1" :class="savingsRate >= 0 ? 'text-green-600' : 'text-red-600'">
            {{ savingsRate.toFixed(1) }}% savings rate
          </p>
        </div>
      </div>

      <!-- Alerts Section -->
      <div v-if="budgetsStore.overBudgetItems.length > 0 || uncategorizedCount > 10" class="space-y-3">
        <!-- Over Budget Alert -->
        <div v-if="budgetsStore.overBudgetItems.length > 0" class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4">
          <div class="flex items-center gap-3">
            <div class="flex-shrink-0">
              <svg class="w-5 h-5 text-red-600 dark:text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            </div>
            <div class="flex-1">
              <h4 class="text-sm font-semibold text-red-800 dark:text-red-200">
                {{ budgetsStore.overBudgetItems.length }} budget{{ budgetsStore.overBudgetItems.length > 1 ? 's' : '' }} over limit
              </h4>
              <p class="text-sm text-red-600 dark:text-red-400">
                {{ budgetsStore.overBudgetItems.map(b => b.category_name).join(', ') }}
              </p>
            </div>
            <button @click="navigateTo('/budgets')" class="text-sm font-medium text-red-600 dark:text-red-400 hover:underline">
              View Budgets
            </button>
          </div>
        </div>

        <!-- Uncategorized Alert -->
        <div v-if="uncategorizedCount > 10" class="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg p-4">
          <div class="flex items-center gap-3">
            <div class="flex-shrink-0">
              <svg class="w-5 h-5 text-yellow-600 dark:text-yellow-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z" />
              </svg>
            </div>
            <div class="flex-1">
              <h4 class="text-sm font-semibold text-yellow-800 dark:text-yellow-200">
                {{ uncategorizedCount }} uncategorized transactions
              </h4>
              <p class="text-sm text-yellow-600 dark:text-yellow-400">
                Categorize them for better budget tracking
              </p>
            </div>
            <button @click="navigateTo('/transactions')" class="text-sm font-medium text-yellow-600 dark:text-yellow-400 hover:underline">
              Categorize Now
            </button>
          </div>
        </div>
      </div>

      <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <!-- Recent Transactions -->
        <div class="lg:col-span-2 bg-white dark:bg-gray-800 rounded-lg shadow">
          <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Recent Transactions</h2>
            <button @click="navigateTo('/transactions')" class="text-sm text-blue-600 hover:text-blue-700 dark:text-blue-400">
              View All
            </button>
          </div>
          <div class="p-4">
            <div v-if="transactionsStore.recentTransactions.length === 0" class="text-center text-gray-500 py-8">
              <svg class="w-12 h-12 mx-auto text-gray-400 mb-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
              </svg>
              <p>No transactions yet</p>
              <button @click="navigateTo('/import')" class="mt-2 text-blue-600 hover:text-blue-700 text-sm font-medium">
                Import your first statement
              </button>
            </div>
            <div v-else class="space-y-3">
              <div
                v-for="tx in transactionsStore.recentTransactions"
                :key="tx.id"
                class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700 last:border-0"
              >
                <div class="flex items-center gap-3">
                  <div
                    class="w-2 h-2 rounded-full"
                    :class="tx.transaction_type === 'income' ? 'bg-green-500' : 'bg-red-500'"
                  />
                  <div>
                    <p class="font-medium text-gray-900 dark:text-white">{{ tx.description }}</p>
                    <p class="text-sm text-gray-500">
                      {{ tx.date }}
                      <span v-if="tx.category_id" class="ml-2 text-gray-400">
                        {{ categoriesStore.categoriesById[tx.category_id]?.name }}
                      </span>
                    </p>
                  </div>
                </div>
                <p
                  :class="[
                    'font-semibold',
                    tx.transaction_type === 'income' ? 'text-green-600' : 'text-red-600'
                  ]"
                >
                  {{ tx.transaction_type === 'income' ? '+' : '-' }}{{ formatCurrency(tx.amount) }}
                </p>
              </div>
            </div>
          </div>
        </div>

        <!-- Sidebar -->
        <div class="space-y-6">
          <!-- Top Spending Categories -->
          <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
            <div class="p-4 border-b border-gray-200 dark:border-gray-700">
              <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Top Spending</h2>
            </div>
            <div class="p-4">
              <div v-if="topCategories.length === 0" class="text-center text-gray-500 py-4">
                No spending data yet
              </div>
              <div v-else class="space-y-3">
                <div
                  v-for="category in topCategories"
                  :key="category.name"
                  class="flex items-center justify-between"
                >
                  <div class="flex items-center gap-2">
                    <div class="w-3 h-3 rounded-full" :style="{ backgroundColor: category.color }" />
                    <span class="text-sm text-gray-700 dark:text-gray-300">{{ category.name }}</span>
                  </div>
                  <span class="text-sm font-medium text-gray-900 dark:text-white">
                    {{ formatCurrency(category.amount) }}
                  </span>
                </div>
              </div>
            </div>
          </div>

          <!-- Upcoming Bills -->
          <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
            <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center">
              <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Upcoming Bills</h2>
              <button @click="navigateTo('/recurring')" class="text-sm text-blue-600 hover:text-blue-700 dark:text-blue-400">
                Manage
              </button>
            </div>
            <div class="p-4">
              <div v-if="calendarStore.upcomingBills.length === 0" class="text-center text-gray-500 py-4">
                <p>No upcoming bills</p>
                <button @click="navigateTo('/recurring')" class="mt-1 text-blue-600 hover:text-blue-700 text-sm">
                  Set up recurring
                </button>
              </div>
              <div v-else class="space-y-3">
                <div
                  v-for="bill in calendarStore.upcomingBills.slice(0, 5)"
                  :key="bill.recurring.id"
                  class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700 last:border-0"
                >
                  <div>
                    <p class="font-medium text-gray-900 dark:text-white text-sm">{{ bill.recurring.description }}</p>
                    <p class="text-xs text-gray-500">
                      <span v-if="bill.days_until === 0" class="text-red-600 font-medium">Due today</span>
                      <span v-else-if="bill.days_until === 1" class="text-orange-600">Tomorrow</span>
                      <span v-else-if="bill.days_until <= 7" class="text-yellow-600">{{ bill.days_until }} days</span>
                      <span v-else>{{ bill.next_date }}</span>
                    </p>
                  </div>
                  <p class="font-semibold text-red-600 text-sm">
                    -{{ formatCurrency(bill.recurring.amount) }}
                  </p>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Budget Progress -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Budget Progress</h2>
          <button @click="navigateTo('/budgets')" class="text-sm text-blue-600 hover:text-blue-700 dark:text-blue-400">
            Manage Budgets
          </button>
        </div>
        <div class="p-4">
          <div v-if="budgetsStore.budgetStatus.length === 0" class="text-center text-gray-500 py-8">
            <svg class="w-12 h-12 mx-auto text-gray-400 mb-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
            </svg>
            <p>No budgets set up yet</p>
            <button @click="navigateTo('/budgets')" class="mt-2 text-blue-600 hover:text-blue-700 text-sm font-medium">
              Create your first budget
            </button>
          </div>
          <div v-else class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            <div
              v-for="status in budgetsStore.budgetStatus"
              :key="status.budget.id"
              class="p-4 rounded-lg border"
              :class="status.is_over_budget ? 'border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/10' : 'border-gray-200 dark:border-gray-700'"
            >
              <div class="flex justify-between items-start mb-2">
                <div class="flex items-center gap-2">
                  <div class="w-3 h-3 rounded-full" :style="{ backgroundColor: status.category_color }" />
                  <span class="font-medium text-gray-900 dark:text-white text-sm">{{ status.category_name }}</span>
                </div>
                <span
                  v-if="status.is_over_budget"
                  class="text-xs font-medium text-red-600 dark:text-red-400 bg-red-100 dark:bg-red-900/30 px-2 py-0.5 rounded-full"
                >
                  Over
                </span>
              </div>
              <div class="flex justify-between text-sm mb-2">
                <span :class="status.is_over_budget ? 'text-red-600' : 'text-gray-600 dark:text-gray-400'">
                  {{ formatCurrency(status.spent) }}
                </span>
                <span class="text-gray-400">/ {{ formatCurrency(status.budget.amount) }}</span>
              </div>
              <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
                <div
                  class="h-2 rounded-full transition-all duration-300"
                  :style="{
                    width: `${Math.min(status.percentage_used, 100)}%`,
                    backgroundColor: status.is_over_budget ? '#ef4444' : status.category_color
                  }"
                />
              </div>
              <p class="text-xs text-gray-500 mt-1">
                {{ status.percentage_used.toFixed(0) }}% used
              </p>
            </div>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
