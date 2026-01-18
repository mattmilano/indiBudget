<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import * as api from '../services/api';
import type { SavingsGoal, CreateGoalRequest, GoalType } from '../types';
import { differenceInDays, parseISO } from 'date-fns';
import ConfirmDialog from '../components/ConfirmDialog.vue';

const goals = ref<SavingsGoal[]>([]);
const showAddModal = ref(false);
const showContributionModal = ref(false);
const showEditModal = ref(false);
const selectedGoal = ref<SavingsGoal | null>(null);
const loading = ref(false);
const contributing = ref(false);

// Delete confirmation
const showDeleteConfirm = ref(false);
const goalToDelete = ref<SavingsGoal | null>(null);

// Contribution form
const contributionAmount = ref('');
const contributionNotes = ref('');

const newGoal = ref<CreateGoalRequest>({
  name: '',
  goal_type: 'savings',
  target_amount: '',
  current_amount: '0',
  target_date: undefined,
  color: '#3b82f6',
  notes: undefined,
});

const editGoal = ref<CreateGoalRequest & { id: string }>({
  id: '',
  name: '',
  goal_type: 'savings',
  target_amount: '',
  current_amount: '0',
  target_date: undefined,
  color: '#3b82f6',
  notes: undefined,
});

const goalTypes: { value: GoalType; label: string; icon: string }[] = [
  { value: 'savings', label: 'Savings', icon: 'M17 9V7a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2m2 4h10a2 2 0 002-2v-6a2 2 0 00-2-2H9a2 2 0 00-2 2v6a2 2 0 002 2zm7-5a2 2 0 11-4 0 2 2 0 014 0z' },
  { value: 'debt_payoff', label: 'Debt Payoff', icon: 'M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z' },
  { value: 'purchase', label: 'Purchase', icon: 'M16 11V7a4 4 0 00-8 0v4M5 9h14l1 12H4L5 9z' },
  { value: 'emergency', label: 'Emergency Fund', icon: 'M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z' },
  { value: 'custom', label: 'Custom', icon: 'M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.143L13 21l-2.286-6.857L5 12l5.714-2.143L13 3z' },
];

const formatCurrency = (value: string | number) => {
  const num = typeof value === 'string' ? parseFloat(value) : value;
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(num || 0);
};

const calculateProgress = (goal: SavingsGoal) => {
  const current = parseFloat(goal.current_amount) || 0;
  const target = parseFloat(goal.target_amount) || 1;
  return Math.min((current / target) * 100, 100);
};

const getRemainingAmount = (goal: SavingsGoal) => {
  const current = parseFloat(goal.current_amount) || 0;
  const target = parseFloat(goal.target_amount) || 0;
  return Math.max(target - current, 0);
};

const getDaysRemaining = (goal: SavingsGoal) => {
  if (!goal.target_date) return null;
  const targetDate = parseISO(goal.target_date);
  const days = differenceInDays(targetDate, new Date());
  return days;
};

const getMonthlyNeeded = (goal: SavingsGoal) => {
  const remaining = getRemainingAmount(goal);
  const days = getDaysRemaining(goal);
  if (!days || days <= 0) return null;
  const months = days / 30;
  return remaining / months;
};

// Sort goals: active first, then by progress
const sortedGoals = computed(() => {
  return [...goals.value].sort((a, b) => {
    // Active goals first
    if (a.status === 'active' && b.status !== 'active') return -1;
    if (b.status === 'active' && a.status !== 'active') return 1;
    // Then by progress (highest first)
    return calculateProgress(b) - calculateProgress(a);
  });
});

async function fetchGoals() {
  loading.value = true;
  try {
    goals.value = await api.getGoals();
  } catch (e) {
    console.error('Failed to fetch goals:', e);
  } finally {
    loading.value = false;
  }
}

async function handleSubmit() {
  try {
    const goal = await api.createGoal(newGoal.value);
    goals.value.push(goal);
    showAddModal.value = false;
    resetForm();
  } catch (e) {
    console.error('Failed to create goal:', e);
  }
}

function resetForm() {
  newGoal.value = {
    name: '',
    goal_type: 'savings',
    target_amount: '',
    current_amount: '0',
    target_date: undefined,
    color: '#3b82f6',
    notes: undefined,
  };
}

function openContributionModal(goal: SavingsGoal) {
  selectedGoal.value = goal;
  contributionAmount.value = '';
  contributionNotes.value = '';
  showContributionModal.value = true;
}

async function handleContribution() {
  if (!selectedGoal.value || !contributionAmount.value) return;

  contributing.value = true;
  try {
    await api.updateGoalProgress(selectedGoal.value.id, contributionAmount.value);
    // Update local state
    const goalIndex = goals.value.findIndex(g => g.id === selectedGoal.value!.id);
    if (goalIndex !== -1) {
      const currentAmount = parseFloat(goals.value[goalIndex].current_amount) || 0;
      const addedAmount = parseFloat(contributionAmount.value) || 0;
      goals.value[goalIndex].current_amount = (currentAmount + addedAmount).toString();

      // Check if goal is completed
      const targetAmount = parseFloat(goals.value[goalIndex].target_amount) || 0;
      if (currentAmount + addedAmount >= targetAmount) {
        goals.value[goalIndex].status = 'completed';
      }
    }
    showContributionModal.value = false;
    selectedGoal.value = null;
  } catch (e) {
    console.error('Failed to add contribution:', e);
    alert('Failed to add contribution.');
  } finally {
    contributing.value = false;
  }
}

function openEditModal(goal: SavingsGoal) {
  selectedGoal.value = goal;
  editGoal.value = {
    id: goal.id,
    name: goal.name,
    goal_type: goal.goal_type,
    target_amount: goal.target_amount,
    current_amount: goal.current_amount,
    target_date: goal.target_date ?? undefined,
    color: goal.color ?? '#3b82f6',
    notes: goal.notes,
  };
  showEditModal.value = true;
}

async function handleEditSubmit() {
  if (!selectedGoal.value) return;

  try {
    // For now, we'll update the local state since there's no update API
    // In a real app, you'd call api.updateGoal(editGoal.value)
    const goalIndex = goals.value.findIndex(g => g.id === editGoal.value.id);
    if (goalIndex !== -1) {
      goals.value[goalIndex] = {
        ...goals.value[goalIndex],
        name: editGoal.value.name,
        goal_type: editGoal.value.goal_type,
        target_amount: editGoal.value.target_amount,
        current_amount: editGoal.value.current_amount ?? goals.value[goalIndex].current_amount,
        target_date: editGoal.value.target_date,
        color: editGoal.value.color ?? goals.value[goalIndex].color,
        notes: editGoal.value.notes,
      };
    }
    showEditModal.value = false;
    selectedGoal.value = null;
  } catch (e) {
    console.error('Failed to update goal:', e);
  }
}

function confirmDeleteGoal(goal: SavingsGoal) {
  goalToDelete.value = goal;
  showDeleteConfirm.value = true;
}

async function handleDeleteConfirm() {
  if (!goalToDelete.value) return;

  try {
    // For now, just remove from local state
    // In a real app, you'd call api.deleteGoal(goalToDelete.value.id)
    goals.value = goals.value.filter(g => g.id !== goalToDelete.value!.id);
    goalToDelete.value = null;
  } catch (e) {
    console.error('Failed to delete goal:', e);
  }
}

function setQuickContribution(percentage: number) {
  if (!selectedGoal.value) return;
  const remaining = getRemainingAmount(selectedGoal.value);
  contributionAmount.value = (remaining * percentage).toFixed(2);
}

onMounted(fetchGoals);
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Savings Goals</h1>
      <button
        @click="showAddModal = true"
        class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors flex items-center gap-2"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6m0 0v6m0-6h6m-6 0H6" />
        </svg>
        Create Goal
      </button>
    </div>

    <!-- Summary Stats -->
    <div v-if="goals.length > 0" class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
        <p class="text-sm text-gray-500 dark:text-gray-400">Total Saved</p>
        <p class="text-2xl font-bold text-green-600">
          {{ formatCurrency(goals.reduce((sum, g) => sum + (parseFloat(g.current_amount) || 0), 0)) }}
        </p>
      </div>
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
        <p class="text-sm text-gray-500 dark:text-gray-400">Total Target</p>
        <p class="text-2xl font-bold text-gray-900 dark:text-white">
          {{ formatCurrency(goals.reduce((sum, g) => sum + (parseFloat(g.target_amount) || 0), 0)) }}
        </p>
      </div>
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
        <p class="text-sm text-gray-500 dark:text-gray-400">Active Goals</p>
        <p class="text-2xl font-bold text-blue-600">
          {{ goals.filter(g => g.status === 'active').length }}
        </p>
      </div>
    </div>

    <!-- Goals Grid -->
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
      <div
        v-for="goal in sortedGoals"
        :key="goal.id"
        class="bg-white dark:bg-gray-800 rounded-lg shadow-lg overflow-hidden group"
      >
        <div class="h-2" :style="{ backgroundColor: goal.color }" />
        <div class="p-6">
          <div class="flex items-start justify-between mb-4">
            <div class="flex-1">
              <h3 class="font-semibold text-lg text-gray-900 dark:text-white">{{ goal.name }}</h3>
              <p class="text-sm text-gray-500 dark:text-gray-400 capitalize">
                {{ goal.goal_type.replace('_', ' ') }}
              </p>
            </div>
            <div class="flex items-center gap-2">
              <span
                :class="[
                  'px-2 py-1 text-xs font-medium rounded-full',
                  goal.status === 'active'
                    ? 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200'
                    : goal.status === 'completed'
                    ? 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200'
                    : 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-200'
                ]"
              >
                {{ goal.status }}
              </span>
              <!-- Action buttons -->
              <div class="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  @click="openEditModal(goal)"
                  class="p-1 text-gray-400 hover:text-blue-600 transition-colors"
                  title="Edit goal"
                >
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                  </svg>
                </button>
                <button
                  @click="confirmDeleteGoal(goal)"
                  class="p-1 text-gray-400 hover:text-red-600 transition-colors"
                  title="Delete goal"
                >
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                  </svg>
                </button>
              </div>
            </div>
          </div>

          <div class="mb-4">
            <div class="flex justify-between text-sm mb-1">
              <span class="text-gray-600 dark:text-gray-400">Progress</span>
              <span class="font-medium text-gray-900 dark:text-white">
                {{ calculateProgress(goal).toFixed(0) }}%
              </span>
            </div>
            <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-3">
              <div
                class="h-3 rounded-full transition-all duration-500"
                :style="{
                  width: `${calculateProgress(goal)}%`,
                  backgroundColor: goal.color
                }"
              />
            </div>
          </div>

          <div class="flex justify-between items-end mb-4">
            <div>
              <p class="text-2xl font-bold text-gray-900 dark:text-white">
                {{ formatCurrency(goal.current_amount) }}
              </p>
              <p class="text-sm text-gray-500 dark:text-gray-400">
                of {{ formatCurrency(goal.target_amount) }}
              </p>
            </div>
            <div v-if="goal.target_date" class="text-right">
              <p class="text-sm text-gray-500 dark:text-gray-400">
                {{ getDaysRemaining(goal) !== null && getDaysRemaining(goal)! > 0 ? `${getDaysRemaining(goal)} days left` : 'Past due' }}
              </p>
              <p class="text-xs text-gray-400">{{ goal.target_date }}</p>
            </div>
          </div>

          <!-- Monthly needed indicator -->
          <div v-if="getMonthlyNeeded(goal) && goal.status === 'active'" class="mb-4 p-2 bg-blue-50 dark:bg-blue-900/30 rounded-lg">
            <p class="text-xs text-blue-600 dark:text-blue-400">
              Save {{ formatCurrency(getMonthlyNeeded(goal)!) }}/month to reach your goal
            </p>
          </div>

          <!-- Remaining amount -->
          <div v-if="getRemainingAmount(goal) > 0" class="mb-4">
            <p class="text-sm text-gray-500 dark:text-gray-400">
              {{ formatCurrency(getRemainingAmount(goal)) }} remaining
            </p>
          </div>

          <p v-if="goal.notes" class="text-sm text-gray-500 dark:text-gray-400 mb-4 line-clamp-2">
            {{ goal.notes }}
          </p>

          <!-- Add Contribution Button -->
          <button
            v-if="goal.status === 'active'"
            @click="openContributionModal(goal)"
            class="w-full py-2 px-4 bg-green-600 text-white rounded-lg hover:bg-green-700 transition-colors flex items-center justify-center gap-2"
          >
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6m0 0v6m0-6h6m-6 0H6" />
            </svg>
            Add Contribution
          </button>

          <!-- Completed badge -->
          <div v-if="goal.status === 'completed'" class="text-center py-2">
            <span class="inline-flex items-center gap-1 text-green-600">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              Goal Achieved!
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- Empty State -->
    <div v-if="goals.length === 0 && !loading" class="text-center py-12">
      <svg class="w-16 h-16 mx-auto text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.143L13 21l-2.286-6.857L5 12l5.714-2.143L13 3z" />
      </svg>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mt-4">No goals yet</h3>
      <p class="text-gray-500 dark:text-gray-400 mt-2">Create your first savings goal to start tracking your progress.</p>
      <button
        @click="showAddModal = true"
        class="mt-4 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
      >
        Create Your First Goal
      </button>
    </div>

    <!-- Add Goal Modal -->
    <div
      v-if="showAddModal"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showAddModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4 max-h-[90vh] overflow-y-auto">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Create Goal</h3>
        </div>
        <form @submit.prevent="handleSubmit" class="p-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Goal Name</label>
            <input
              v-model="newGoal.name"
              type="text"
              required
              placeholder="e.g., Vacation Fund"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Goal Type</label>
            <select
              v-model="newGoal.goal_type"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            >
              <option v-for="type in goalTypes" :key="type.value" :value="type.value">
                {{ type.label }}
              </option>
            </select>
          </div>
          <div class="grid grid-cols-2 gap-4">
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Target Amount</label>
              <input
                v-model="newGoal.target_amount"
                type="number"
                step="0.01"
                min="0"
                required
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
            </div>
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Current Amount</label>
              <input
                v-model="newGoal.current_amount"
                type="number"
                step="0.01"
                min="0"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
            </div>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Target Date (optional)</label>
            <input
              v-model="newGoal.target_date"
              type="date"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Color</label>
            <input
              v-model="newGoal.color"
              type="color"
              class="w-full h-10 border border-gray-300 dark:border-gray-600 rounded-lg cursor-pointer"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Notes (optional)</label>
            <textarea
              v-model="newGoal.notes"
              rows="2"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
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
              Create Goal
            </button>
          </div>
        </form>
      </div>
    </div>

    <!-- Add Contribution Modal -->
    <div
      v-if="showContributionModal && selectedGoal"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showContributionModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Add Contribution</h3>
          <p class="text-sm text-gray-500 dark:text-gray-400">{{ selectedGoal.name }}</p>
        </div>
        <div class="p-4 space-y-4">
          <!-- Current progress -->
          <div class="p-3 bg-gray-50 dark:bg-gray-700 rounded-lg">
            <div class="flex justify-between text-sm mb-2">
              <span class="text-gray-600 dark:text-gray-400">Current Progress</span>
              <span class="font-medium text-gray-900 dark:text-white">{{ calculateProgress(selectedGoal).toFixed(0) }}%</span>
            </div>
            <div class="w-full bg-gray-200 dark:bg-gray-600 rounded-full h-2">
              <div
                class="h-2 rounded-full"
                :style="{ width: `${calculateProgress(selectedGoal)}%`, backgroundColor: selectedGoal.color }"
              />
            </div>
            <p class="text-sm text-gray-500 dark:text-gray-400 mt-2">
              {{ formatCurrency(selectedGoal.current_amount) }} of {{ formatCurrency(selectedGoal.target_amount) }}
            </p>
          </div>

          <!-- Contribution amount -->
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Contribution Amount
            </label>
            <div class="relative">
              <span class="absolute left-3 top-2 text-gray-500">$</span>
              <input
                v-model="contributionAmount"
                type="number"
                step="0.01"
                min="0.01"
                required
                placeholder="0.00"
                class="w-full pl-7 pr-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-lg"
              />
            </div>
          </div>

          <!-- Quick amount buttons -->
          <div class="flex gap-2">
            <button
              type="button"
              @click="setQuickContribution(0.1)"
              class="flex-1 py-1.5 px-2 text-sm bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
            >
              10%
            </button>
            <button
              type="button"
              @click="setQuickContribution(0.25)"
              class="flex-1 py-1.5 px-2 text-sm bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
            >
              25%
            </button>
            <button
              type="button"
              @click="setQuickContribution(0.5)"
              class="flex-1 py-1.5 px-2 text-sm bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
            >
              50%
            </button>
            <button
              type="button"
              @click="setQuickContribution(1)"
              class="flex-1 py-1.5 px-2 text-sm bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300 rounded hover:bg-green-200 dark:hover:bg-green-800 transition-colors"
            >
              Full
            </button>
          </div>

          <!-- Remaining info -->
          <p class="text-sm text-gray-500 dark:text-gray-400">
            {{ formatCurrency(getRemainingAmount(selectedGoal)) }} remaining to reach your goal
          </p>

          <!-- Notes -->
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Notes (optional)
            </label>
            <input
              v-model="contributionNotes"
              type="text"
              placeholder="e.g., Bonus deposit"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
        </div>
        <div class="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-3">
          <button
            @click="showContributionModal = false"
            class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
          >
            Cancel
          </button>
          <button
            @click="handleContribution"
            :disabled="!contributionAmount || contributing"
            class="px-4 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
          >
            <svg v-if="contributing" class="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            {{ contributing ? 'Adding...' : 'Add Contribution' }}
          </button>
        </div>
      </div>
    </div>

    <!-- Edit Goal Modal -->
    <div
      v-if="showEditModal && selectedGoal"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showEditModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4 max-h-[90vh] overflow-y-auto">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Edit Goal</h3>
        </div>
        <form @submit.prevent="handleEditSubmit" class="p-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Goal Name</label>
            <input
              v-model="editGoal.name"
              type="text"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Goal Type</label>
            <select
              v-model="editGoal.goal_type"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            >
              <option v-for="type in goalTypes" :key="type.value" :value="type.value">
                {{ type.label }}
              </option>
            </select>
          </div>
          <div class="grid grid-cols-2 gap-4">
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Target Amount</label>
              <input
                v-model="editGoal.target_amount"
                type="number"
                step="0.01"
                min="0"
                required
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
            </div>
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Current Amount</label>
              <input
                v-model="editGoal.current_amount"
                type="number"
                step="0.01"
                min="0"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
            </div>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Target Date (optional)</label>
            <input
              v-model="editGoal.target_date"
              type="date"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Color</label>
            <input
              v-model="editGoal.color"
              type="color"
              class="w-full h-10 border border-gray-300 dark:border-gray-600 rounded-lg cursor-pointer"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Notes (optional)</label>
            <textarea
              v-model="editGoal.notes"
              rows="2"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
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
      title="Delete Goal"
      :message="`Are you sure you want to delete '${goalToDelete?.name}'? This action cannot be undone.`"
      confirmText="Delete"
      variant="danger"
      @confirm="handleDeleteConfirm"
      @cancel="showDeleteConfirm = false; goalToDelete = null"
      @update:show="showDeleteConfirm = $event"
    />
  </div>
</template>
