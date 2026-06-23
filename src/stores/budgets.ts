import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { Budget, CreateBudgetRequest, BudgetStatus } from '../types';
import * as api from '../services/api';

type UpdateBudgetPayload = Partial<Budget> & { id: string };

export const useBudgetsStore = defineStore('budgets', () => {
  const budgets = ref<Budget[]>([]);
  const budgetStatus = ref<BudgetStatus[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const activeBudgets = computed(() => budgets.value.filter(b => b.is_active));

  const overBudgetItems = computed(() =>
    budgetStatus.value.filter(s => s.is_over_budget)
  );

  const totalBudgeted = computed(() => {
    return budgetStatus.value.reduce((sum, s) => {
      return sum + (parseFloat(s.budget.amount) || 0);
    }, 0);
  });

  const totalSpent = computed(() => {
    return budgetStatus.value.reduce((sum, s) => {
      return sum + (parseFloat(s.spent) || 0);
    }, 0);
  });

  async function fetchBudgets() {
    loading.value = true;
    error.value = null;
    try {
      budgets.value = await api.getBudgets();
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  async function fetchBudgetStatus(asOfDate?: string) {
    loading.value = true;
    error.value = null;
    try {
      budgetStatus.value = await api.getBudgetStatus(asOfDate);
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  async function createBudget(request: CreateBudgetRequest) {
    loading.value = true;
    error.value = null;
    try {
      const budget = await api.createBudget(request);
      budgets.value.push(budget);
      await fetchBudgetStatus();
      return budget;
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function updateBudget(request: UpdateBudgetPayload) {
    loading.value = true;
    error.value = null;
    try {
      const budget = await api.updateBudget(request);
      const index = budgets.value.findIndex(b => b.id === budget.id);
      if (index !== -1) {
        budgets.value[index] = budget;
      }
      await fetchBudgetStatus();
      return budget;
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function deleteBudget(id: string) {
    loading.value = true;
    error.value = null;
    try {
      await api.deleteBudget(id);
      budgets.value = budgets.value.filter(b => b.id !== id);
      await fetchBudgetStatus();
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  return {
    budgets,
    budgetStatus,
    loading,
    error,
    activeBudgets,
    overBudgetItems,
    totalBudgeted,
    totalSpent,
    fetchBudgets,
    fetchBudgetStatus,
    createBudget,
    updateBudget,
    deleteBudget,
  };
});
