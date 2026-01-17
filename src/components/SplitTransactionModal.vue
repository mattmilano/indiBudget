<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useCategoriesStore } from '../stores';
import type { Transaction, SplitPart } from '../types';

interface Props {
  show: boolean;
  transaction: Transaction | null;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'save', parts: SplitPart[]): void;
}>();

const categoriesStore = useCategoriesStore();

interface SplitPartForm {
  amount: string;
  category_id: string;
  description: string;
}

const parts = ref<SplitPartForm[]>([
  { amount: '', category_id: '', description: '' },
  { amount: '', category_id: '', description: '' },
]);

const formatCurrency = (value: number | string) => {
  const num = typeof value === 'string' ? parseFloat(value) : value;
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(num || 0);
};

const transactionAmount = computed(() => {
  if (!props.transaction) return 0;
  return parseFloat(props.transaction.amount) || 0;
});

const allocatedAmount = computed(() => {
  return parts.value.reduce((sum, part) => {
    const amount = parseFloat(part.amount) || 0;
    return sum + amount;
  }, 0);
});

const remainingAmount = computed(() => {
  return transactionAmount.value - allocatedAmount.value;
});

const isBalanced = computed(() => {
  return Math.abs(remainingAmount.value) < 0.01;
});

const canSave = computed(() => {
  if (!isBalanced.value) return false;
  return parts.value.every(part => {
    const amount = parseFloat(part.amount) || 0;
    return amount > 0;
  });
});

const expenseCategories = computed(() =>
  categoriesStore.categories.filter(c => c.category_type === 'expense' && c.is_active)
);

const incomeCategories = computed(() =>
  categoriesStore.categories.filter(c => c.category_type === 'income' && c.is_active)
);

const availableCategories = computed(() => {
  if (!props.transaction) return [];
  return props.transaction.transaction_type === 'income' ? incomeCategories.value : expenseCategories.value;
});

function addPart() {
  parts.value.push({ amount: '', category_id: '', description: '' });
}

function removePart(index: number) {
  if (parts.value.length > 2) {
    parts.value.splice(index, 1);
  }
}

function distributeEvenly() {
  const partCount = parts.value.length;
  const evenAmount = (transactionAmount.value / partCount).toFixed(2);
  const remainder = transactionAmount.value - (parseFloat(evenAmount) * partCount);

  parts.value.forEach((part, index) => {
    if (index === 0) {
      part.amount = (parseFloat(evenAmount) + remainder).toFixed(2);
    } else {
      part.amount = evenAmount;
    }
  });
}

function assignRemaining(index: number) {
  const currentAmount = parseFloat(parts.value[index].amount) || 0;
  parts.value[index].amount = (currentAmount + remainingAmount.value).toFixed(2);
}

function handleSave() {
  if (!canSave.value) return;

  const splitParts: SplitPart[] = parts.value.map(part => ({
    amount: part.amount,
    category_id: part.category_id || undefined,
    description: part.description || undefined,
  }));

  emit('save', splitParts);
}

function handleClose() {
  emit('close');
}

// Reset form when modal opens with a new transaction
watch(() => props.show, (newVal) => {
  if (newVal && props.transaction) {
    const halfAmount = (transactionAmount.value / 2).toFixed(2);
    parts.value = [
      { amount: halfAmount, category_id: props.transaction.category_id || '', description: '' },
      { amount: halfAmount, category_id: '', description: '' },
    ];
  }
});
</script>

<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition ease-out duration-200"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="transition ease-in duration-150"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="show && transaction"
        class="fixed inset-0 z-50 overflow-y-auto"
        @click.self="handleClose"
      >
        <div class="flex min-h-full items-center justify-center p-4">
          <!-- Backdrop -->
          <div class="fixed inset-0 bg-black/50 transition-opacity" @click="handleClose" />

          <!-- Modal -->
          <div class="relative bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-2xl w-full mx-4 overflow-hidden">
            <!-- Header -->
            <div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
              <div class="flex items-center justify-between">
                <div>
                  <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
                    Split Transaction
                  </h3>
                  <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
                    {{ transaction.description }} - {{ formatCurrency(transaction.amount) }}
                  </p>
                </div>
                <button
                  @click="handleClose"
                  class="text-gray-400 hover:text-gray-500 dark:hover:text-gray-300"
                >
                  <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
            </div>

            <!-- Body -->
            <div class="px-6 py-4 max-h-[60vh] overflow-y-auto">
              <!-- Balance indicator -->
              <div :class="[
                'mb-4 p-3 rounded-lg flex items-center justify-between',
                isBalanced
                  ? 'bg-green-100 dark:bg-green-900/30 text-green-800 dark:text-green-200'
                  : 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-800 dark:text-yellow-200'
              ]">
                <div class="flex items-center gap-2">
                  <svg v-if="isBalanced" class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                  </svg>
                  <svg v-else class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                  </svg>
                  <span class="font-medium">
                    {{ isBalanced ? 'Balanced' : `Remaining: ${formatCurrency(remainingAmount)}` }}
                  </span>
                </div>
                <span class="text-sm">
                  Allocated: {{ formatCurrency(allocatedAmount) }} of {{ formatCurrency(transactionAmount) }}
                </span>
              </div>

              <!-- Quick actions -->
              <div class="mb-4 flex gap-2">
                <button
                  @click="distributeEvenly"
                  class="px-3 py-1.5 text-sm font-medium text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/20 rounded-lg transition-colors"
                >
                  Distribute Evenly
                </button>
                <button
                  @click="addPart"
                  class="px-3 py-1.5 text-sm font-medium text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
                >
                  + Add Part
                </button>
              </div>

              <!-- Split parts -->
              <div class="space-y-4">
                <div
                  v-for="(part, index) in parts"
                  :key="index"
                  class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4"
                >
                  <div class="flex items-center justify-between mb-3">
                    <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
                      Part {{ index + 1 }}
                    </span>
                    <div class="flex items-center gap-2">
                      <button
                        v-if="!isBalanced && remainingAmount !== 0"
                        @click="assignRemaining(index)"
                        class="text-xs text-blue-600 dark:text-blue-400 hover:underline"
                      >
                        + Add remaining
                      </button>
                      <button
                        v-if="parts.length > 2"
                        @click="removePart(index)"
                        class="text-gray-400 hover:text-red-500"
                      >
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                        </svg>
                      </button>
                    </div>
                  </div>

                  <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
                    <!-- Amount -->
                    <div>
                      <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
                        Amount
                      </label>
                      <div class="relative">
                        <span class="absolute left-3 top-1/2 -translate-y-1/2 text-gray-500">$</span>
                        <input
                          v-model="part.amount"
                          type="number"
                          step="0.01"
                          min="0"
                          :max="transactionAmount"
                          class="w-full pl-7 pr-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                          placeholder="0.00"
                        />
                      </div>
                    </div>

                    <!-- Category -->
                    <div>
                      <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
                        Category
                      </label>
                      <select
                        v-model="part.category_id"
                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                      >
                        <option value="">Uncategorized</option>
                        <option v-for="cat in availableCategories" :key="cat.id" :value="cat.id">
                          {{ cat.name }}
                        </option>
                      </select>
                    </div>

                    <!-- Description -->
                    <div>
                      <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
                        Description (optional)
                      </label>
                      <input
                        v-model="part.description"
                        type="text"
                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                        :placeholder="transaction.description"
                      />
                    </div>
                  </div>
                </div>
              </div>
            </div>

            <!-- Footer -->
            <div class="px-6 py-4 bg-gray-50 dark:bg-gray-700/50 flex justify-end gap-3">
              <button
                type="button"
                @click="handleClose"
                class="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-gray-500 transition-colors"
              >
                Cancel
              </button>
              <button
                type="button"
                @click="handleSave"
                :disabled="!canSave"
                :class="[
                  'px-4 py-2 text-sm font-medium text-white rounded-lg focus:outline-none focus:ring-2 focus:ring-offset-2 transition-colors',
                  canSave
                    ? 'bg-blue-600 hover:bg-blue-700 focus:ring-blue-500'
                    : 'bg-gray-400 cursor-not-allowed'
                ]"
              >
                Split Transaction
              </button>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
