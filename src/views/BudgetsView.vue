<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { useBudgetsStore, useCategoriesStore } from '../stores';
import type { CreateBudgetRequest, BudgetPeriod, BudgetStatus } from '../types';
import { format, startOfMonth } from 'date-fns';
import ConfirmDialog from '../components/ConfirmDialog.vue';

const budgetsStore = useBudgetsStore();
const categoriesStore = useCategoriesStore();

const showAddModal = ref(false);
const showEditModal = ref(false);
const showTemplateModal = ref(false);
const showDeleteConfirm = ref(false);
const applyingTemplate = ref(false);
const monthlyIncome = ref('5000');

const newBudget = ref<CreateBudgetRequest>({
  name: '',
  category_id: '',
  amount: '',
  period: 'monthly',
  start_date: format(startOfMonth(new Date()), 'yyyy-MM-dd'),
  rollover: false,
});

// Budget being edited (a budget's category is fixed once created)
const editingBudget = ref<BudgetStatus | null>(null);
const editForm = ref({
  name: '',
  amount: '',
  period: 'monthly' as BudgetPeriod,
  start_date: format(startOfMonth(new Date()), 'yyyy-MM-dd'),
  rollover: false,
});

// Budget pending deletion
const budgetToDelete = ref<BudgetStatus | null>(null);

const periodOptions: { value: BudgetPeriod; label: string }[] = [
  { value: 'weekly', label: 'Weekly' },
  { value: 'biweekly', label: 'Bi-weekly' },
  { value: 'monthly', label: 'Monthly' },
  { value: 'quarterly', label: 'Quarterly' },
  { value: 'yearly', label: 'Yearly' },
];

// Budget templates
interface BudgetTemplate {
  id: string;
  name: string;
  description: string;
  icon: string;
  categories: { name: string; percentage: number }[];
}

const budgetTemplates: BudgetTemplate[] = [
  {
    id: '50-30-20',
    name: '50/30/20 Rule',
    description: 'Popular rule: 50% needs, 30% wants, 20% savings',
    icon: 'M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z',
    categories: [
      { name: 'Housing', percentage: 25 },
      { name: 'Transportation', percentage: 10 },
      { name: 'Utilities', percentage: 5 },
      { name: 'Groceries', percentage: 10 },
      { name: 'Entertainment', percentage: 10 },
      { name: 'Dining Out', percentage: 10 },
      { name: 'Shopping', percentage: 10 },
      { name: 'Savings', percentage: 20 },
    ],
  },
  {
    id: 'zero-based',
    name: 'Zero-Based Budget',
    description: 'Every dollar has a job - detailed category tracking',
    icon: 'M9 7h6m0 10v-3m-3 3h.01M9 17h.01M9 14h.01M12 14h.01M15 11h.01M12 11h.01M9 11h.01M7 21h10a2 2 0 002-2V5a2 2 0 00-2-2H7a2 2 0 00-2 2v14a2 2 0 002 2z',
    categories: [
      { name: 'Housing', percentage: 25 },
      { name: 'Utilities', percentage: 5 },
      { name: 'Groceries', percentage: 10 },
      { name: 'Transportation', percentage: 10 },
      { name: 'Insurance', percentage: 5 },
      { name: 'Healthcare', percentage: 5 },
      { name: 'Dining Out', percentage: 5 },
      { name: 'Entertainment', percentage: 5 },
      { name: 'Personal Care', percentage: 3 },
      { name: 'Clothing', percentage: 3 },
      { name: 'Education', percentage: 4 },
      { name: 'Savings', percentage: 15 },
      { name: 'Debt Payments', percentage: 5 },
    ],
  },
  {
    id: 'minimalist',
    name: 'Minimalist Budget',
    description: 'Simple categories for straightforward tracking',
    icon: 'M4 5a1 1 0 011-1h14a1 1 0 011 1v2a1 1 0 01-1 1H5a1 1 0 01-1-1V5zM4 13a1 1 0 011-1h6a1 1 0 011 1v6a1 1 0 01-1 1H5a1 1 0 01-1-1v-6zM16 13a1 1 0 011-1h2a1 1 0 011 1v6a1 1 0 01-1 1h-2a1 1 0 01-1-1v-6z',
    categories: [
      { name: 'Fixed Expenses', percentage: 50 },
      { name: 'Variable Expenses', percentage: 30 },
      { name: 'Savings', percentage: 20 },
    ],
  },
  {
    id: 'family',
    name: 'Family Budget',
    description: 'Comprehensive budget for households with children',
    icon: 'M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z',
    categories: [
      { name: 'Housing', percentage: 28 },
      { name: 'Groceries', percentage: 12 },
      { name: 'Transportation', percentage: 10 },
      { name: 'Utilities', percentage: 5 },
      { name: 'Childcare', percentage: 10 },
      { name: 'Healthcare', percentage: 5 },
      { name: 'Education', percentage: 5 },
      { name: 'Entertainment', percentage: 5 },
      { name: 'Savings', percentage: 15 },
      { name: 'Emergency Fund', percentage: 5 },
    ],
  },
];

const selectedTemplate = ref<BudgetTemplate | null>(null);

// Computed template preview amounts
const templatePreview = computed(() => {
  if (!selectedTemplate.value) return [];
  const income = parseFloat(monthlyIncome.value) || 0;
  return selectedTemplate.value.categories.map(cat => ({
    name: cat.name,
    percentage: cat.percentage,
    amount: (income * cat.percentage) / 100,
  }));
});

// Find matching category by name
function findCategoryByName(name: string) {
  const normalizedName = name.toLowerCase();
  return categoriesStore.expenseCategories.find(cat =>
    cat.name.toLowerCase().includes(normalizedName) ||
    normalizedName.includes(cat.name.toLowerCase())
  );
}

// Apply template
async function applyTemplate() {
  if (!selectedTemplate.value) return;

  applyingTemplate.value = true;
  const income = parseFloat(monthlyIncome.value) || 0;
  const startDate = format(startOfMonth(new Date()), 'yyyy-MM-dd');

  try {
    for (const cat of selectedTemplate.value.categories) {
      const matchingCategory = findCategoryByName(cat.name);
      if (matchingCategory) {
        const amount = ((income * cat.percentage) / 100).toFixed(2);
        await budgetsStore.createBudget({
          name: `${cat.name} Budget`,
          category_id: matchingCategory.id,
          amount,
          period: 'monthly',
          start_date: startDate,
          rollover: false,
        });
      }
    }

    showTemplateModal.value = false;
    selectedTemplate.value = null;
    alert('Budget template applied successfully!');
  } catch (e) {
    console.error('Failed to apply template:', e);
    alert('Failed to apply template. Some budgets may have been created.');
  } finally {
    applyingTemplate.value = false;
  }
}

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

function openEditModal(status: BudgetStatus) {
  editingBudget.value = status;
  editForm.value = {
    name: status.budget.name,
    amount: status.budget.amount,
    period: status.budget.period,
    start_date: status.budget.start_date,
    rollover: status.budget.rollover,
  };
  showEditModal.value = true;
}

async function handleEditSubmit() {
  if (!editingBudget.value) return;
  try {
    await budgetsStore.updateBudget({
      id: editingBudget.value.budget.id,
      name: editForm.value.name,
      amount: editForm.value.amount,
      period: editForm.value.period,
      start_date: editForm.value.start_date,
      rollover: editForm.value.rollover,
    });
    showEditModal.value = false;
    editingBudget.value = null;
  } catch (e) {
    console.error('Failed to update budget:', e);
    alert('Failed to update budget. Please try again.');
  }
}

function confirmDelete(status: BudgetStatus) {
  budgetToDelete.value = status;
  showDeleteConfirm.value = true;
}

async function handleDeleteBudget() {
  if (!budgetToDelete.value) return;
  try {
    await budgetsStore.deleteBudget(budgetToDelete.value.budget.id);
    showDeleteConfirm.value = false;
    budgetToDelete.value = null;
  } catch (e) {
    console.error('Failed to delete budget:', e);
    alert('Failed to delete budget. Please try again.');
  }
}

onMounted(async () => {
  await Promise.all([categoriesStore.fetchCategories(), budgetsStore.fetchBudgetStatus()]);
});
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Budgets</h1>
      <div class="flex gap-3">
        <button
          @click="showTemplateModal = true"
          class="px-4 py-2 border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors flex items-center gap-2"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 5a1 1 0 011-1h14a1 1 0 011 1v2a1 1 0 01-1 1H5a1 1 0 01-1-1V5zM4 13a1 1 0 011-1h6a1 1 0 011 1v6a1 1 0 01-1 1H5a1 1 0 01-1-1v-6zM16 13a1 1 0 011-1h2a1 1 0 011 1v6a1 1 0 01-1 1h-2a1 1 0 01-1-1v-6z" />
          </svg>
          Use Template
        </button>
        <button
          @click="showAddModal = true"
          class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          Create Budget
        </button>
      </div>
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
              <h3 class="font-semibold text-gray-900 dark:text-white">{{ status.budget.name }}</h3>
              <p class="text-sm text-gray-500 dark:text-gray-400 capitalize">
                {{ status.category_name }} · {{ status.budget.period }} · {{ status.period_start }} to {{ status.period_end }}
              </p>
            </div>
          </div>
          <div class="flex items-center gap-3">
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
            <div class="flex gap-1">
              <button
                @click="openEditModal(status)"
                class="p-1.5 text-gray-400 hover:text-blue-600 transition-colors"
                title="Edit budget"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                </svg>
              </button>
              <button
                @click="confirmDelete(status)"
                class="p-1.5 text-gray-400 hover:text-red-600 transition-colors"
                title="Delete budget"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
              </button>
            </div>
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

    <!-- Edit Budget Modal -->
    <div
      v-if="showEditModal && editingBudget"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showEditModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Edit Budget</h3>
          <p class="text-sm text-gray-500 dark:text-gray-400">{{ editingBudget.category_name }}</p>
        </div>
        <form @submit.prevent="handleEditSubmit" class="p-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Budget Name</label>
            <input
              v-model="editForm.name"
              type="text"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Amount</label>
            <input
              v-model="editForm.amount"
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
              v-model="editForm.period"
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
              v-model="editForm.start_date"
              type="date"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
          <div class="flex items-center gap-2">
            <input
              v-model="editForm.rollover"
              type="checkbox"
              id="edit-rollover"
              class="rounded border-gray-300 dark:border-gray-600"
            />
            <label for="edit-rollover" class="text-sm text-gray-700 dark:text-gray-300">
              Roll over unused budget to next period
            </label>
          </div>
          <div class="flex justify-end gap-3 pt-4">
            <button
              type="button"
              @click="showEditModal = false"
              class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
            >
              Save Changes
            </button>
          </div>
        </form>
      </div>
    </div>

    <!-- Delete Confirmation -->
    <ConfirmDialog
      :show="showDeleteConfirm"
      title="Delete Budget"
      :message="`Are you sure you want to delete the '${budgetToDelete?.budget.name}' budget? This action cannot be undone.`"
      confirm-text="Delete"
      cancel-text="Cancel"
      variant="danger"
      @confirm="handleDeleteBudget"
      @cancel="showDeleteConfirm = false; budgetToDelete = null"
      @update:show="showDeleteConfirm = $event"
    />

    <!-- Budget Template Modal -->
    <div
      v-if="showTemplateModal"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showTemplateModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-2xl w-full mx-4 max-h-[90vh] overflow-hidden flex flex-col">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center gap-3">
          <div class="p-2 bg-purple-100 dark:bg-purple-900 rounded-lg">
            <svg class="w-5 h-5 text-purple-600 dark:text-purple-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 5a1 1 0 011-1h14a1 1 0 011 1v2a1 1 0 01-1 1H5a1 1 0 01-1-1V5zM4 13a1 1 0 011-1h6a1 1 0 011 1v6a1 1 0 01-1 1H5a1 1 0 01-1-1v-6zM16 13a1 1 0 011-1h2a1 1 0 011 1v6a1 1 0 01-1 1h-2a1 1 0 01-1-1v-6z" />
            </svg>
          </div>
          <div>
            <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Budget Templates</h3>
            <p class="text-sm text-gray-500">Choose a template to quickly set up your budgets</p>
          </div>
        </div>

        <div class="p-4 overflow-y-auto flex-1">
          <!-- Income Input -->
          <div class="mb-6 p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
            <label class="block text-sm font-medium text-blue-800 dark:text-blue-300 mb-2">
              Monthly Income (for budget calculations)
            </label>
            <div class="flex items-center gap-2">
              <span class="text-lg text-blue-600 dark:text-blue-400">$</span>
              <input
                v-model="monthlyIncome"
                type="number"
                min="0"
                step="100"
                class="flex-1 px-3 py-2 border border-blue-300 dark:border-blue-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
              />
            </div>
          </div>

          <!-- Template Selection -->
          <div v-if="!selectedTemplate" class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div
              v-for="template in budgetTemplates"
              :key="template.id"
              @click="selectedTemplate = template"
              class="p-4 border-2 border-gray-200 dark:border-gray-600 rounded-lg hover:border-purple-500 dark:hover:border-purple-400 cursor-pointer transition-colors"
            >
              <div class="flex items-start gap-3">
                <div class="p-2 bg-purple-100 dark:bg-purple-900/30 rounded-lg flex-shrink-0">
                  <svg class="w-5 h-5 text-purple-600 dark:text-purple-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" :d="template.icon" />
                  </svg>
                </div>
                <div>
                  <h4 class="font-semibold text-gray-900 dark:text-white">{{ template.name }}</h4>
                  <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{{ template.description }}</p>
                  <p class="text-xs text-gray-400 mt-2">{{ template.categories.length }} categories</p>
                </div>
              </div>
            </div>
          </div>

          <!-- Template Preview -->
          <div v-else>
            <button
              @click="selectedTemplate = null"
              class="mb-4 text-sm text-purple-600 dark:text-purple-400 hover:underline flex items-center gap-1"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
              </svg>
              Back to templates
            </button>

            <div class="p-4 bg-purple-50 dark:bg-purple-900/20 rounded-lg mb-4">
              <div class="flex items-center gap-3">
                <svg class="w-6 h-6 text-purple-600 dark:text-purple-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" :d="selectedTemplate.icon" />
                </svg>
                <div>
                  <h4 class="font-semibold text-purple-800 dark:text-purple-300">{{ selectedTemplate.name }}</h4>
                  <p class="text-sm text-purple-600 dark:text-purple-400">{{ selectedTemplate.description }}</p>
                </div>
              </div>
            </div>

            <h5 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Budget Breakdown:</h5>
            <div class="space-y-2 max-h-64 overflow-y-auto">
              <div
                v-for="item in templatePreview"
                :key="item.name"
                class="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-700 rounded-lg"
              >
                <div class="flex items-center gap-3">
                  <div class="w-8 h-8 rounded-full bg-purple-100 dark:bg-purple-900/30 flex items-center justify-center">
                    <span class="text-xs font-medium text-purple-600 dark:text-purple-400">{{ item.percentage }}%</span>
                  </div>
                  <span class="font-medium text-gray-900 dark:text-white">{{ item.name }}</span>
                </div>
                <span class="font-semibold text-gray-900 dark:text-white">{{ formatCurrency(item.amount) }}</span>
              </div>
            </div>

            <div class="mt-4 p-3 bg-green-50 dark:bg-green-900/20 rounded-lg">
              <div class="flex justify-between text-sm">
                <span class="text-green-700 dark:text-green-400">Total Budgeted:</span>
                <span class="font-semibold text-green-700 dark:text-green-400">{{ formatCurrency(parseFloat(monthlyIncome) || 0) }}</span>
              </div>
            </div>

            <p class="mt-4 text-xs text-gray-500 dark:text-gray-400">
              Note: Budgets will only be created for categories that match your existing expense categories.
            </p>
          </div>
        </div>

        <div class="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-3">
          <button
            @click="showTemplateModal = false; selectedTemplate = null"
            :disabled="applyingTemplate"
            class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            v-if="selectedTemplate"
            @click="applyTemplate"
            :disabled="applyingTemplate"
            class="px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 transition-colors disabled:opacity-50 flex items-center gap-2"
          >
            <svg v-if="applyingTemplate" class="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            {{ applyingTemplate ? 'Applying...' : 'Apply Template' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
