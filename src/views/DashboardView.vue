<script setup lang="ts">
import { onMounted, computed } from 'vue';
import { useAccountsStore, useTransactionsStore, useBudgetsStore, useCalendarStore } from '../stores';
import { format, startOfMonth, endOfMonth } from 'date-fns';

const accountsStore = useAccountsStore();
const transactionsStore = useTransactionsStore();
const budgetsStore = useBudgetsStore();
const calendarStore = useCalendarStore();

const today = new Date();
const monthStart = format(startOfMonth(today), 'yyyy-MM-dd');
const monthEnd = format(endOfMonth(today), 'yyyy-MM-dd');

onMounted(async () => {
  await Promise.all([
    accountsStore.fetchAccounts(),
    transactionsStore.fetchTransactions({ start_date: monthStart, end_date: monthEnd }),
    budgetsStore.fetchBudgetStatus(),
    calendarStore.fetchUpcomingRecurring(30),
  ]);
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
</script>

<template>
  <div class="p-6 space-y-6">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Dashboard</h1>

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
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Budget Status</h3>
        <p class="text-2xl font-bold text-gray-900 dark:text-white mt-2">
          {{ formatCurrency(budgetsStore.totalSpent) }} / {{ formatCurrency(budgetsStore.totalBudgeted) }}
        </p>
        <p class="text-sm text-gray-500 mt-1">
          {{ budgetsStore.overBudgetItems.length }} over budget
        </p>
      </div>
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <!-- Recent Transactions -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Recent Transactions</h2>
        </div>
        <div class="p-4">
          <div v-if="transactionsStore.recentTransactions.length === 0" class="text-center text-gray-500 py-8">
            No transactions yet. Import your first statement!
          </div>
          <div v-else class="space-y-3">
            <div
              v-for="tx in transactionsStore.recentTransactions"
              :key="tx.id"
              class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700 last:border-0"
            >
              <div>
                <p class="font-medium text-gray-900 dark:text-white">{{ tx.description }}</p>
                <p class="text-sm text-gray-500">{{ tx.date }}</p>
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

      <!-- Upcoming Bills -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Upcoming Bills</h2>
        </div>
        <div class="p-4">
          <div v-if="calendarStore.upcomingBills.length === 0" class="text-center text-gray-500 py-8">
            No upcoming bills. Set up recurring transactions!
          </div>
          <div v-else class="space-y-3">
            <div
              v-for="bill in calendarStore.upcomingBills.slice(0, 5)"
              :key="bill.recurring.id"
              class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700 last:border-0"
            >
              <div>
                <p class="font-medium text-gray-900 dark:text-white">{{ bill.recurring.description }}</p>
                <p class="text-sm text-gray-500">
                  {{ bill.next_date }} ({{ bill.days_until }} days)
                </p>
              </div>
              <p class="font-semibold text-red-600">
                -{{ formatCurrency(bill.recurring.amount) }}
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Budget Progress -->
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
      <div class="p-4 border-b border-gray-200 dark:border-gray-700">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Budget Progress</h2>
      </div>
      <div class="p-4">
        <div v-if="budgetsStore.budgetStatus.length === 0" class="text-center text-gray-500 py-8">
          No budgets set up yet. Create your first budget!
        </div>
        <div v-else class="space-y-4">
          <div
            v-for="status in budgetsStore.budgetStatus"
            :key="status.budget.id"
            class="space-y-2"
          >
            <div class="flex justify-between text-sm">
              <span class="font-medium text-gray-900 dark:text-white">{{ status.category_name }}</span>
              <span :class="status.is_over_budget ? 'text-red-600' : 'text-gray-600'">
                {{ formatCurrency(status.spent) }} / {{ formatCurrency(status.budget.amount) }}
              </span>
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
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
