<script setup lang="ts">
import { ref, onMounted } from 'vue';
import * as api from '../services/api';
import type { SavingsGoal, CreateGoalRequest, GoalType } from '../types';

const goals = ref<SavingsGoal[]>([]);
const showAddModal = ref(false);
const loading = ref(false);

const newGoal = ref<CreateGoalRequest>({
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

onMounted(fetchGoals);
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Savings Goals</h1>
      <button
        @click="showAddModal = true"
        class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
      >
        Create Goal
      </button>
    </div>

    <!-- Goals Grid -->
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
      <div
        v-for="goal in goals"
        :key="goal.id"
        class="bg-white dark:bg-gray-800 rounded-lg shadow-lg overflow-hidden"
      >
        <div class="h-2" :style="{ backgroundColor: goal.color }" />
        <div class="p-6">
          <div class="flex items-start justify-between mb-4">
            <div>
              <h3 class="font-semibold text-lg text-gray-900 dark:text-white">{{ goal.name }}</h3>
              <p class="text-sm text-gray-500 dark:text-gray-400 capitalize">
                {{ goal.goal_type.replace('_', ' ') }}
              </p>
            </div>
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

          <div class="flex justify-between items-end">
            <div>
              <p class="text-2xl font-bold text-gray-900 dark:text-white">
                {{ formatCurrency(goal.current_amount) }}
              </p>
              <p class="text-sm text-gray-500 dark:text-gray-400">
                of {{ formatCurrency(goal.target_amount) }}
              </p>
            </div>
            <div v-if="goal.target_date" class="text-right">
              <p class="text-sm text-gray-500 dark:text-gray-400">Target Date</p>
              <p class="font-medium text-gray-900 dark:text-white">{{ goal.target_date }}</p>
            </div>
          </div>

          <p v-if="goal.notes" class="mt-4 text-sm text-gray-500 dark:text-gray-400">
            {{ goal.notes }}
          </p>
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
    </div>

    <!-- Add Goal Modal -->
    <div
      v-if="showAddModal"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showAddModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4">
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
  </div>
</template>
