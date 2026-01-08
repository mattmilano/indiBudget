import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { Transaction, CreateTransactionRequest, TransactionFilter } from '../types';
import * as api from '../services/api';

export const useTransactionsStore = defineStore('transactions', () => {
  const transactions = ref<Transaction[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const filter = ref<TransactionFilter>({});

  const sortedTransactions = computed(() => {
    return [...transactions.value].sort((a, b) => {
      const dateA = new Date(a.date).getTime();
      const dateB = new Date(b.date).getTime();
      return dateB - dateA;
    });
  });

  const recentTransactions = computed(() => sortedTransactions.value.slice(0, 10));

  const transactionsByDate = computed(() => {
    const grouped: Record<string, Transaction[]> = {};
    for (const tx of transactions.value) {
      if (!grouped[tx.date]) {
        grouped[tx.date] = [];
      }
      grouped[tx.date].push(tx);
    }
    return grouped;
  });

  async function fetchTransactions(newFilter?: TransactionFilter) {
    if (newFilter) {
      filter.value = newFilter;
    }
    loading.value = true;
    error.value = null;
    try {
      console.log('Fetching transactions with filter:', JSON.stringify(filter.value));
      const result = await api.getTransactions(filter.value);
      console.log('Fetched transactions count:', result.length);
      transactions.value = result;
    } catch (e) {
      console.error('Failed to fetch transactions:', e);
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  async function createTransaction(request: CreateTransactionRequest) {
    loading.value = true;
    error.value = null;
    try {
      const transaction = await api.createTransaction(request);
      transactions.value.unshift(transaction);
      return transaction;
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function updateTransaction(request: Partial<Transaction> & { id: string }) {
    loading.value = true;
    error.value = null;
    try {
      const updated = await api.updateTransaction(request);
      const index = transactions.value.findIndex(t => t.id === updated.id);
      if (index !== -1) {
        transactions.value[index] = updated;
      }
      return updated;
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function deleteTransaction(id: string) {
    loading.value = true;
    error.value = null;
    try {
      await api.deleteTransaction(id);
      transactions.value = transactions.value.filter(t => t.id !== id);
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  function setFilter(newFilter: TransactionFilter) {
    filter.value = newFilter;
  }

  function clearFilter() {
    filter.value = {};
  }

  return {
    transactions,
    loading,
    error,
    filter,
    sortedTransactions,
    recentTransactions,
    transactionsByDate,
    fetchTransactions,
    createTransaction,
    updateTransaction,
    deleteTransaction,
    setFilter,
    clearFilter,
  };
});
