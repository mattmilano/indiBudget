<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useAccountsStore } from '../stores';
import type { Account, CreateAccountRequest, AccountType } from '../types';
import ConfirmDialog from '../components/ConfirmDialog.vue';

const accountsStore = useAccountsStore();

const showAddModal = ref(false);
const showEditModal = ref(false);
const showDeleteConfirm = ref(false);
const accountToDelete = ref<Account | null>(null);
const editingAccount = ref<Account | null>(null);

const newAccount = ref<CreateAccountRequest>({
  name: '',
  account_type: 'checking',
  balance: '0',
  currency: 'USD',
  institution: undefined,
  account_number_last4: undefined,
});

const editForm = ref({
  name: '',
  account_type: 'checking' as AccountType,
  balance: '',
  institution: '',
  account_number_last4: '',
});

const accountTypes: { value: AccountType; label: string }[] = [
  { value: 'checking', label: 'Checking' },
  { value: 'savings', label: 'Savings' },
  { value: 'credit_card', label: 'Credit Card' },
  { value: 'cash', label: 'Cash' },
  { value: 'investment', label: 'Investment' },
  { value: 'loan', label: 'Loan' },
  { value: 'other', label: 'Other' },
];

const formatCurrency = (value: string | number) => {
  const num = typeof value === 'string' ? parseFloat(value) : value;
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(num || 0);
};

const getAccountTypeIcon = (type: AccountType) => {
  switch (type) {
    case 'checking':
      return 'M3 6l3 1m0 0l-3 9a5.002 5.002 0 006.001 0M6 7l3 9M6 7l6-2m6 2l3-1m-3 1l-3 9a5.002 5.002 0 006.001 0M18 7l3 9m-3-9l-6-2m0-2v2m0 16V5m0 16H9m3 0h3';
    case 'savings':
      return 'M17 9V7a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2m2 4h10a2 2 0 002-2v-6a2 2 0 00-2-2H9a2 2 0 00-2 2v6a2 2 0 002 2zm7-5a2 2 0 11-4 0 2 2 0 014 0z';
    case 'credit_card':
      return 'M3 10h18M7 15h1m4 0h1m-7 4h12a3 3 0 003-3V8a3 3 0 00-3-3H6a3 3 0 00-3 3v8a3 3 0 003 3z';
    case 'cash':
      return 'M17 9V7a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2m2 4h10a2 2 0 002-2v-6a2 2 0 00-2-2H9a2 2 0 00-2 2v6a2 2 0 002 2zm7-5a2 2 0 11-4 0 2 2 0 014 0z';
    case 'investment':
      return 'M13 7h8m0 0v8m0-8l-8 8-4-4-6 6';
    case 'loan':
      return 'M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z';
    default:
      return 'M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4';
  }
};

async function handleSubmit() {
  if (!newAccount.value.name.trim()) {
    return;
  }
  try {
    await accountsStore.createAccount(newAccount.value);
    showAddModal.value = false;
    resetForm();
  } catch (e) {
    console.error('Failed to create account:', e);
  }
}

function resetForm() {
  newAccount.value = {
    name: '',
    account_type: 'checking',
    balance: '0',
    currency: 'USD',
    institution: undefined,
    account_number_last4: undefined,
  };
}

function openEditModal(account: Account) {
  editingAccount.value = account;
  editForm.value = {
    name: account.name,
    account_type: account.account_type,
    balance: account.balance,
    institution: account.institution || '',
    account_number_last4: account.account_number_last4 || '',
  };
  showEditModal.value = true;
}

async function handleEditSubmit() {
  if (!editingAccount.value || !editForm.value.name.trim()) {
    return;
  }
  try {
    await accountsStore.updateAccount({
      id: editingAccount.value.id,
      name: editForm.value.name,
      account_type: editForm.value.account_type,
      balance: editForm.value.balance,
      institution: editForm.value.institution || undefined,
      account_number_last4: editForm.value.account_number_last4 || undefined,
    });
    showEditModal.value = false;
    editingAccount.value = null;
  } catch (e) {
    console.error('Failed to update account:', e);
  }
}

function confirmDelete(account: Account) {
  accountToDelete.value = account;
  showDeleteConfirm.value = true;
}

async function deleteAccount() {
  if (!accountToDelete.value) return;
  try {
    await accountsStore.deleteAccount(accountToDelete.value.id);
    showDeleteConfirm.value = false;
    accountToDelete.value = null;
  } catch (e) {
    console.error('Failed to delete account:', e);
  }
}

onMounted(() => {
  accountsStore.fetchAccounts();
});
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Accounts</h1>
      <button
        @click="showAddModal = true"
        class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors flex items-center gap-2"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
        </svg>
        Add Account
      </button>
    </div>

    <!-- Total Balance Card -->
    <div class="bg-gradient-to-r from-blue-600 to-blue-700 rounded-lg shadow-lg p-6 mb-6 text-white">
      <h2 class="text-lg opacity-90">Total Net Worth</h2>
      <p class="text-4xl font-bold mt-2">{{ formatCurrency(accountsStore.totalBalance) }}</p>
      <p class="text-sm opacity-75 mt-2">Across {{ accountsStore.activeAccounts.length }} accounts</p>
    </div>

    <!-- Accounts Grid -->
    <div v-if="accountsStore.accounts.length > 0" class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      <div
        v-for="account in accountsStore.accounts"
        :key="account.id"
        class="bg-white dark:bg-gray-800 rounded-lg shadow p-6 hover:shadow-md transition-shadow"
      >
        <div class="flex items-start justify-between">
          <div class="flex items-center gap-3">
            <div class="w-12 h-12 rounded-full bg-blue-100 dark:bg-blue-900 flex items-center justify-center">
              <svg class="w-6 h-6 text-blue-600 dark:text-blue-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" :d="getAccountTypeIcon(account.account_type)" />
              </svg>
            </div>
            <div>
              <h3 class="font-semibold text-gray-900 dark:text-white">{{ account.name }}</h3>
              <p class="text-sm text-gray-500 dark:text-gray-400 capitalize">
                {{ account.account_type.replace('_', ' ') }}
              </p>
            </div>
          </div>
          <div class="flex gap-1">
            <button
              @click="openEditModal(account)"
              class="p-2 text-gray-400 hover:text-blue-600 transition-colors"
              title="Edit account"
            >
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
              </svg>
            </button>
            <button
              @click="confirmDelete(account)"
              class="p-2 text-gray-400 hover:text-red-600 transition-colors"
              title="Delete account"
            >
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
          </div>
        </div>
        <div class="mt-4">
          <p
            :class="[
              'text-2xl font-bold',
              account.account_type === 'credit_card' || account.account_type === 'loan'
                ? 'text-red-600'
                : 'text-gray-900 dark:text-white'
            ]"
          >
            {{ formatCurrency(account.balance) }}
          </p>
          <p v-if="account.institution" class="text-sm text-gray-500 dark:text-gray-400 mt-1">
            {{ account.institution }}
            <span v-if="account.account_number_last4"> &middot; ****{{ account.account_number_last4 }}</span>
          </p>
        </div>
      </div>
    </div>

    <!-- Empty State -->
    <div v-else class="bg-white dark:bg-gray-800 rounded-lg shadow p-12 text-center">
      <svg class="w-16 h-16 mx-auto text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 6l3 1m0 0l-3 9a5.002 5.002 0 006.001 0M6 7l3 9M6 7l6-2m6 2l3-1m-3 1l-3 9a5.002 5.002 0 006.001 0M18 7l3 9m-3-9l-6-2m0-2v2m0 16V5m0 16H9m3 0h3" />
      </svg>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mt-4">No accounts yet</h3>
      <p class="text-gray-500 dark:text-gray-400 mt-2 mb-4">Add your first account to start tracking your finances.</p>
      <button
        @click="showAddModal = true"
        class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
      >
        Add Your First Account
      </button>
    </div>

    <!-- Add Account Modal -->
    <div
      v-if="showAddModal"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showAddModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Add Account</h3>
          <button @click="showAddModal = false" class="text-gray-400 hover:text-gray-600">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <form @submit.prevent="handleSubmit" class="p-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Account Name <span class="text-red-500">*</span>
            </label>
            <input
              v-model="newAccount.name"
              type="text"
              required
              placeholder="e.g., Main Checking"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Account Type</label>
            <select
              v-model="newAccount.account_type"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
            >
              <option v-for="type in accountTypes" :key="type.value" :value="type.value">
                {{ type.label }}
              </option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Current Balance</label>
            <div class="relative">
              <span class="absolute left-3 top-2 text-gray-500">$</span>
              <input
                v-model="newAccount.balance"
                type="number"
                step="0.01"
                required
                class="w-full pl-7 pr-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
              />
            </div>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Institution (optional)</label>
            <input
              v-model="newAccount.institution"
              type="text"
              placeholder="e.g., Chase Bank"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Last 4 digits (optional)</label>
            <input
              v-model="newAccount.account_number_last4"
              type="text"
              maxlength="4"
              placeholder="1234"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
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
              Add Account
            </button>
          </div>
        </form>
      </div>
    </div>

    <!-- Edit Account Modal -->
    <div
      v-if="showEditModal && editingAccount"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showEditModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Edit Account</h3>
          <button @click="showEditModal = false" class="text-gray-400 hover:text-gray-600">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <form @submit.prevent="handleEditSubmit" class="p-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Account Name <span class="text-red-500">*</span>
            </label>
            <input
              v-model="editForm.name"
              type="text"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Account Type</label>
            <select
              v-model="editForm.account_type"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
            >
              <option v-for="type in accountTypes" :key="type.value" :value="type.value">
                {{ type.label }}
              </option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Current Balance
              <span class="text-xs text-gray-500 ml-1">(adjust if needed)</span>
            </label>
            <div class="relative">
              <span class="absolute left-3 top-2 text-gray-500">$</span>
              <input
                v-model="editForm.balance"
                type="number"
                step="0.01"
                required
                class="w-full pl-7 pr-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
              />
            </div>
            <p class="text-xs text-gray-500 mt-1">
              Use this to correct the balance or make manual adjustments
            </p>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Institution</label>
            <input
              v-model="editForm.institution"
              type="text"
              placeholder="e.g., Chase Bank"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Last 4 digits</label>
            <input
              v-model="editForm.account_number_last4"
              type="text"
              maxlength="4"
              placeholder="1234"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
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
      title="Delete Account"
      :message="`Are you sure you want to delete '${accountToDelete?.name}'? This will not delete associated transactions, but they will no longer be linked to this account.`"
      confirm-text="Delete"
      cancel-text="Cancel"
      variant="danger"
      @confirm="deleteAccount"
      @cancel="showDeleteConfirm = false"
      @update:show="showDeleteConfirm = $event"
    />
  </div>
</template>
