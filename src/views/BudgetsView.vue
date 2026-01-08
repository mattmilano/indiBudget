<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useBudgetsStore, useCategoriesStore } from '../stores';
import type { CreateBudgetRequest, BudgetPeriod } from '../types';
import { format, startOfMonth } from 'date-fns';

const budgetsStore = useBudgetsStore();
const categoriesStore = useCategoriesStore();

const showAddModal = ref(false);
const newBudget = ref<CreateBudgetRequest>({
  name: '',
  category_id: '',
  amount: '',
  period: 'monthly',
  start_date: format(startOfMonth(new Date()), 'yyyy-MM-dd'),
  rollover: false,
});

const periodOptions: { value: BudgetPeriod; label: string }[] = [
  { value: 'weekly', label: 'Weekly' },
  { value: 'biweekly', label: 'Bi-weekly' },
  { value: 'monthly', label: 'Monthly' },
  { value: 'quarterly', label: 'Quarterly' },
  { value: 'yearly', label: 'Yearly' },
];

const formatCurrency = (value: string | number) => {
  const num = typeof value === 'string' ? parseFloat(value) : value;
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(num || 0);
};

async function handleSubmit() {
  try {
    await budgetsStore.createBudget(newBudget.value);
    showAddModal.value = false;
    resetForm();
  } catch (e) {
    console.error('Failed to create budget:', e);
  }
}

function resetForm() {
  newBudget.value = {
    name: '',
    category_id: '',
    amount: '',
    period: 'monthly',
    start_date: format(startOfMonth(new Date()), 'yyyy-MM-dd'),
    rollover: false,
  };
}

onMounted(async () => {
  await Promise.all([categoriesStore.fetchCategories(), budgetsStore.fetchBudgetStatus()]);
});
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Budgets</h1>
      <button
        @click="showAddModal = true"
        class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
      >
        Create Budget
      </button>
    </div>

    <!-- Summary Cards -->
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Total Budgeted</h3>
        <p class="text-2xl font-bold text-gray-900 dark:text-white mt-2">
          {{ formatCurrency(budgetsStore.totalBudgeted) }}
        </p>
      </div>
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Total Spent</h3>
        <p class="text-2xl font-bold text-gray-900 dark:text-white mt-2">
          {{ formatCurrency(budgetsStore.totalSpent) }}
        </p>
      </div>
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Remaining</h3>
        <p
          :class="[
            'text-2xl font-bold mt-2',
            budgetsStore.totalBudgeted - budgetsStore.totalSpent >= 0
              ? 'text-green-600'
              : 'text-red-600'
          ]"
        >
          {{ formatCurrency(budgetsStore.totalBudgeted - budgetsStore.totalSpent) }}
        </p>
      </div>
    </div>

    <!-- Budget List -->
    <div class="space-y-4">
      <div
        v-for="status in budgetsStore.budgetStatus"
        :key="status.budget.id"
        class="bg-white dark:bg-gray-800 rounded-lg shadow p-6"
      >
        <div class="flex items-center justify-between mb-4">
          <div class="flex items-center gap-3">
            <div
              class="w-4 h-4 rounded-full"
              :style="{ backgroundColor: status.category_color }"
            />
            <div>
              <h3 class="font-semibold text-gray-900 dark:text-white">{{ status.category_name }}</h3>
              <p class="text-sm text-gray-500 dark:text-gray-400 capitalize">
                {{ status.budget.period }} · {{ status.period_start }} to {{ status.period_end }}
              </p>
            </div>
          </div>
          <div class="text-right">
            <p
              :class="[
                'text-lg font-bold',
                status.is_over_budget ? 'text-red-600' : 'text-gray-900 dark:text-white'
              ]"
            >
              {{ formatCurrency(status.spent) }} / {{ formatCurrency(status.budget.amount) }}
            </p>
            <p
              :class="[
                'text-sm',
                status.is_over_budget ? 'text-red-500' : 'text-green-500'
              ]"
            >
              {{ status.is_over_budget ? 'Over by' : 'Remaining:' }}
              {{ formatCurrency(Math.abs(parseFloat(status.remaining))) }}
            </p>
          </div>
        </div>
        <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-3">
          <div
            class="h-3 rounded-full transition-all duration-500"
            :style="{
              width: `${Math.min(status.percentage_used, 100)}%`,
              backgroundColor: status.is_over_budget ? '#ef4444' : status.category_color
            }"
          />
        </div>
        <p class="text-sm text-gray-500 dark:text-gray-400 mt-2">
          {{ status.percentage_used.toFixed(1) }}% used
        </p>
      </div>
    </div>

    <!-- Empty State -->
    <div v-if="budgetsStore.budgetStatus.length === 0" class="text-center py-12">
      <svg class="w-16 h-16 mx-auto text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
      </svg>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mt-4">No budgets yet</h3>
      <p class="text-gray-500 dark:text-gray-400 mt-2">Create your first budget to start tracking your spending.</p>
    </div>

    <!-- Add Budget Modal -->
    <div
      v-if="showAddModal"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showAddModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Create Budget</h3>
        </div>
        <form @submit.prevent="handleSubmit" class="p-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Category</label>
            <select
              v-model="newBudget.category_id"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            >
              <option value="">Select a category</option>
              <option v-for="cat in categoriesStore.expenseCategories" :key="cat.id" :value="cat.id">
                {{ cat.name }}
              </option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Budget Name</label>
            <input
              v-model="newBudget.name"
              type="text"
              required
              placeholder="e.g., Monthly Groceries"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Amount</label>
            <input
              v-model="newBudget.amount"
              type="number"
              step="0.01"
              min="0"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Period</label>
            <select
              v-model="newBudget.period"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            >
              <option v-for="period in periodOptions" :key="period.value" :value="period.value">
                {{ period.label }}
              </option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Start Date</label>
            <input
              v-model="newBudget.start_date"
              type="date"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
          <div class="flex items-center gap-2">
            <input
              v-model="newBudget.rollover"
              type="checkbox"
              id="rollover"
              class="rounded border-gray-300 dark:border-gray-600"
            />
            <label for="rollover" class="text-sm text-gray-700 dark:text-gray-300">
              Roll over unused budget to next period
            </label>
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
              Create Budget
            </button>
          </div>
        </form>
      </div>
    </div>
  </div>
</template>
