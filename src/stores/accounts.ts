import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { Account, CreateAccountRequest } from '../types';
import * as api from '../services/api';

export const useAccountsStore = defineStore('accounts', () => {
  const accounts = ref<Account[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const totalBalance = computed(() => {
    return accounts.value.reduce((sum, acc) => {
      const balance = parseFloat(acc.balance) || 0;
      return acc.account_type === 'credit_card' || acc.account_type === 'loan'
        ? sum - balance
        : sum + balance;
    }, 0);
  });

  const activeAccounts = computed(() => accounts.value.filter(a => a.is_active));

  const accountsById = computed(() => {
    return accounts.value.reduce((map, acc) => {
      map[acc.id] = acc;
      return map;
    }, {} as Record<string, Account>);
  });

  async function fetchAccounts() {
    loading.value = true;
    error.value = null;
    try {
      const result = await api.getAccounts();
      accounts.value = result;
    } catch (e) {
      console.error('Failed to fetch accounts:', e);
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  async function createAccount(request: CreateAccountRequest) {
    loading.value = true;
    error.value = null;
    try {
      const account = await api.createAccount(request);
      accounts.value.push(account);
      return account;
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function updateAccount(request: Partial<Account> & { id: string }) {
    loading.value = true;
    error.value = null;
    try {
      const updated = await api.updateAccount(request);
      const index = accounts.value.findIndex(a => a.id === updated.id);
      if (index !== -1) {
        accounts.value[index] = updated;
      }
      return updated;
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function deleteAccount(id: string) {
    loading.value = true;
    error.value = null;
    try {
      await api.deleteAccount(id);
      accounts.value = accounts.value.filter(a => a.id !== id);
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  return {
    accounts,
    loading,
    error,
    totalBalance,
    activeAccounts,
    accountsById,
    fetchAccounts,
    createAccount,
    updateAccount,
    deleteAccount,
  };
});
