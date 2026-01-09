<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { open } from '@tauri-apps/plugin-dialog';
import { useAccountsStore, useTransactionsStore } from '../stores';
import * as api from '../services/api';
import type { ImportMapping, RawTransaction, ImportResult } from '../types';

const router = useRouter();
const accountsStore = useAccountsStore();
const transactionsStore = useTransactionsStore();

const selectedFile = ref<string | null>(null);
const selectedAccountId = ref('');
const columns = ref<string[]>([]);
const previewData = ref<RawTransaction[]>([]);
const importResult = ref<ImportResult | null>(null);
const loading = ref(false);
const errorMessage = ref('');
const step = ref<'select' | 'map' | 'preview' | 'result'>('select');

const mapping = ref<ImportMapping>({
  date_column: 'Date',
  description_column: 'Description',
  amount_column: 'Amount',
  debit_column: undefined,
  credit_column: undefined,
  category_column: undefined,
  date_format: '%m/%d/%Y',
  has_header: true,
  skip_rows: 0,
});

const dateFormats = [
  { value: '%m/%d/%Y', label: 'MM/DD/YYYY (01/31/2024)' },
  { value: '%Y-%m-%d', label: 'YYYY-MM-DD (2024-01-31)' },
  { value: '%d/%m/%Y', label: 'DD/MM/YYYY (31/01/2024)' },
  { value: '%m-%d-%Y', label: 'MM-DD-YYYY (01-31-2024)' },
  { value: '%Y/%m/%d', label: 'YYYY/MM/DD (2024/01/31)' },
];

const useSeparateColumns = ref(false);

const isAutoFormat = ref(false);

async function selectFile() {
  const selected = await open({
    multiple: false,
    filters: [
      { name: 'All Supported', extensions: ['csv', 'xlsx', 'xls', 'ofx', 'qfx', 'qif'] },
      { name: 'CSV', extensions: ['csv'] },
      { name: 'Excel', extensions: ['xlsx', 'xls'] },
      { name: 'OFX/QFX (Bank)', extensions: ['ofx', 'qfx'] },
      { name: 'QIF (Quicken)', extensions: ['qif'] },
    ],
  });

  if (selected && typeof selected === 'string') {
    selectedFile.value = selected;
    try {
      loading.value = true;
      columns.value = await api.detectImportColumns(selected);

      // Check if this is an auto-format (OFX/QFX/QIF) that doesn't need column mapping
      if (columns.value.length === 1 && columns.value[0] === '__AUTO__') {
        isAutoFormat.value = true;
        // Skip column mapping step and go directly to preview
        await previewImport();
      } else if (columns.value.length > 0) {
        isAutoFormat.value = false;
        autoMapColumns();
        step.value = 'map';
      }
    } catch (e) {
      console.error('Failed to detect columns:', e);
      errorMessage.value = `Failed to read file: ${e}`;
    } finally {
      loading.value = false;
    }
  }
}

function autoMapColumns() {
  const lowerColumns = columns.value.map(c => c.toLowerCase());

  const dateCol = columns.value.find((_, i) =>
    lowerColumns[i].includes('date') || lowerColumns[i].includes('posted')
  );
  if (dateCol) mapping.value.date_column = dateCol;

  const descCol = columns.value.find((_, i) =>
    lowerColumns[i].includes('description') ||
    lowerColumns[i].includes('memo') ||
    lowerColumns[i].includes('payee') ||
    lowerColumns[i].includes('name')
  );
  if (descCol) mapping.value.description_column = descCol;

  const amountCol = columns.value.find((_, i) => lowerColumns[i].includes('amount'));
  if (amountCol) mapping.value.amount_column = amountCol;

  const debitCol = columns.value.find((_, i) =>
    lowerColumns[i].includes('debit') || lowerColumns[i].includes('withdrawal')
  );
  const creditCol = columns.value.find((_, i) =>
    lowerColumns[i].includes('credit') || lowerColumns[i].includes('deposit')
  );

  if (debitCol && creditCol) {
    useSeparateColumns.value = true;
    mapping.value.debit_column = debitCol;
    mapping.value.credit_column = creditCol;
  }
}

async function previewImport() {
  if (!selectedFile.value) return;

  try {
    loading.value = true;
    const mappingToSend = { ...mapping.value };
    if (!useSeparateColumns.value) {
      mappingToSend.debit_column = undefined;
      mappingToSend.credit_column = undefined;
    }
    previewData.value = await api.previewImport(selectedFile.value, mappingToSend);
    step.value = 'preview';
  } catch (e) {
    console.error('Failed to preview import:', e);
    errorMessage.value = `Failed to preview: ${e}`;
  } finally {
    loading.value = false;
  }
}

async function performImport() {
  if (!selectedFile.value || !selectedAccountId.value) {
    errorMessage.value = 'Please select a file and account first';
    return;
  }

  try {
    loading.value = true;
    errorMessage.value = '';
    const mappingToSend = { ...mapping.value };
    if (!useSeparateColumns.value) {
      mappingToSend.debit_column = undefined;
      mappingToSend.credit_column = undefined;
    }
    importResult.value = await api.importTransactions(
      selectedFile.value,
      selectedAccountId.value,
      mappingToSend
    );
    step.value = 'result';
    // Refresh the transactions store with no filter so all views see the imported data
    await transactionsStore.fetchTransactions({});
  } catch (e) {
    console.error('Failed to import:', e);
    errorMessage.value = `Failed to import: ${e}`;
  } finally {
    loading.value = false;
  }
}

function viewTransactions() {
  router.push('/transactions');
}

function resetImport() {
  selectedFile.value = null;
  columns.value = [];
  previewData.value = [];
  importResult.value = null;
  errorMessage.value = '';
  step.value = 'select';
  isAutoFormat.value = false;
  mapping.value = {
    date_column: 'Date',
    description_column: 'Description',
    amount_column: 'Amount',
    debit_column: undefined,
    credit_column: undefined,
    category_column: undefined,
    date_format: '%m/%d/%Y',
    has_header: true,
    skip_rows: 0,
  };
}

onMounted(() => {
  accountsStore.fetchAccounts();
});
</script>

<template>
  <div class="p-6">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white mb-6">Import Transactions</h1>

    <!-- Error Message -->
    <div v-if="errorMessage" class="mb-4 p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
      <p class="text-red-700 dark:text-red-400">{{ errorMessage }}</p>
      <button @click="errorMessage = ''" class="mt-2 text-sm text-red-600 dark:text-red-500 underline">Dismiss</button>
    </div>

    <!-- Step 1: Select File -->
    <div v-if="step === 'select'" class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
      <h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Select File</h2>
      <p class="text-gray-500 dark:text-gray-400 mb-6">
        Choose a CSV or Excel file to import transactions from your bank statement.
      </p>

      <div class="mb-6">
        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Account</label>
        <select
          v-model="selectedAccountId"
          required
          class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
        >
          <option value="">Select an account</option>
          <option v-for="account in accountsStore.accounts" :key="account.id" :value="account.id">
            {{ account.name }}
          </option>
        </select>
      </div>

      <button
        @click="selectFile"
        :disabled="!selectedAccountId || loading"
        class="w-full py-12 border-2 border-dashed border-gray-300 dark:border-gray-600 rounded-lg text-center hover:border-blue-500 hover:bg-blue-50 dark:hover:bg-blue-900/20 transition-colors disabled:opacity-50"
      >
        <svg class="w-12 h-12 mx-auto text-gray-400 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
        </svg>
        <p class="text-gray-600 dark:text-gray-400">Click to select a file</p>
        <p class="text-sm text-gray-500 dark:text-gray-500 mt-1">CSV, Excel, OFX, QFX, or QIF</p>
      </button>
    </div>

    <!-- Step 2: Map Columns -->
    <div v-if="step === 'map'" class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
      <h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Map Columns</h2>
      <p class="text-gray-500 dark:text-gray-400 mb-6">
        Match your file's columns to the transaction fields.
      </p>

      <div class="space-y-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Date Column</label>
          <select
            v-model="mapping.date_column"
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
          >
            <option v-for="col in columns" :key="col" :value="col">{{ col }}</option>
          </select>
        </div>

        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Date Format</label>
          <select
            v-model="mapping.date_format"
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
          >
            <option v-for="fmt in dateFormats" :key="fmt.value" :value="fmt.value">{{ fmt.label }}</option>
          </select>
        </div>

        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Description Column</label>
          <select
            v-model="mapping.description_column"
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
          >
            <option v-for="col in columns" :key="col" :value="col">{{ col }}</option>
          </select>
        </div>

        <div class="flex items-center gap-2 my-4">
          <input
            v-model="useSeparateColumns"
            type="checkbox"
            id="separateColumns"
            class="rounded border-gray-300 dark:border-gray-600"
          />
          <label for="separateColumns" class="text-sm text-gray-700 dark:text-gray-300">
            Use separate debit/credit columns
          </label>
        </div>

        <div v-if="!useSeparateColumns">
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Amount Column</label>
          <select
            v-model="mapping.amount_column"
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
          >
            <option v-for="col in columns" :key="col" :value="col">{{ col }}</option>
          </select>
        </div>

        <div v-else class="grid grid-cols-2 gap-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Debit Column</label>
            <select
              v-model="mapping.debit_column"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            >
              <option v-for="col in columns" :key="col" :value="col">{{ col }}</option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Credit Column</label>
            <select
              v-model="mapping.credit_column"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            >
              <option v-for="col in columns" :key="col" :value="col">{{ col }}</option>
            </select>
          </div>
        </div>
      </div>

      <div class="flex justify-between mt-6">
        <button
          @click="step = 'select'"
          class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
        >
          Back
        </button>
        <button
          @click="previewImport"
          :disabled="loading"
          class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50"
        >
          Preview Import
        </button>
      </div>
    </div>

    <!-- Step 3: Preview -->
    <div v-if="step === 'preview'" class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
      <h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Preview</h2>
      <p class="text-gray-500 dark:text-gray-400 mb-6">
        Review the first 10 transactions before importing.
      </p>

      <div class="overflow-x-auto">
        <table class="w-full text-sm">
          <thead>
            <tr class="border-b border-gray-200 dark:border-gray-700">
              <th class="text-left py-2 px-4">Date</th>
              <th class="text-left py-2 px-4">Description</th>
              <th class="text-right py-2 px-4">Amount</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="(tx, index) in previewData"
              :key="index"
              class="border-b border-gray-100 dark:border-gray-700"
            >
              <td class="py-2 px-4">{{ tx.date }}</td>
              <td class="py-2 px-4">{{ tx.description }}</td>
              <td class="py-2 px-4 text-right" :class="parseFloat(tx.amount) < 0 ? 'text-red-600' : 'text-green-600'">
                {{ tx.amount }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="flex justify-between mt-6">
        <button
          @click="step = isAutoFormat ? 'select' : 'map'"
          class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
        >
          Back
        </button>
        <button
          @click="performImport"
          :disabled="loading"
          class="px-4 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 transition-colors disabled:opacity-50"
        >
          Import All Transactions
        </button>
      </div>
    </div>

    <!-- Loading Overlay -->
    <div
      v-if="loading"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
    >
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-2xl p-8 max-w-sm w-full mx-4 text-center">
        <div class="relative w-16 h-16 mx-auto mb-4">
          <svg class="animate-spin w-16 h-16 text-blue-600" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
          </svg>
        </div>
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">
          {{ step === 'preview' ? 'Importing Transactions...' : 'Processing File...' }}
        </h3>
        <p class="text-sm text-gray-500 dark:text-gray-400">
          {{ step === 'preview'
            ? 'Importing and categorizing your transactions. This may take a moment for large files.'
            : 'Reading and parsing your file. Please wait...'
          }}
        </p>
      </div>
    </div>

    <!-- Step 4: Result -->
    <div v-if="step === 'result' && importResult" class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
      <div class="text-center">
        <svg class="w-16 h-16 mx-auto text-green-500 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Import Complete!</h2>
        <p class="text-gray-500 dark:text-gray-400 mb-6">
          Your transactions have been imported successfully.
        </p>

        <div class="grid grid-cols-3 gap-4 max-w-md mx-auto mb-6">
          <div class="bg-green-50 dark:bg-green-900/20 rounded-lg p-4">
            <p class="text-3xl font-bold text-green-600">{{ importResult.imported.length }}</p>
            <p class="text-sm text-green-700 dark:text-green-400">Imported</p>
          </div>
          <div class="bg-yellow-50 dark:bg-yellow-900/20 rounded-lg p-4">
            <p class="text-3xl font-bold text-yellow-600">{{ importResult.skipped_duplicates }}</p>
            <p class="text-sm text-yellow-700 dark:text-yellow-400">Duplicates</p>
          </div>
          <div class="bg-red-50 dark:bg-red-900/20 rounded-lg p-4">
            <p class="text-3xl font-bold text-red-600">{{ importResult.errors.length }}</p>
            <p class="text-sm text-red-700 dark:text-red-400">Errors</p>
          </div>
        </div>

        <div class="flex justify-center gap-4">
          <button
            @click="viewTransactions"
            class="px-6 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 transition-colors"
          >
            View Transactions
          </button>
          <button
            @click="resetImport"
            class="px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
          >
            Import Another File
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
