<script setup lang="ts">
/**
 * Edit a scheduled transaction in place.
 *
 * Opened from the Recurring screen and from the calendar, so it fetches the
 * record itself by id rather than trusting whatever copy the caller had. The
 * save carries the version it was opened with: if someone else changed the
 * schedule meanwhile, the save is refused with everything typed kept.
 */
import { computed, ref, watch } from 'vue';
import * as api from '../services/api';
import { useAccountsStore, useCategoriesStore } from '../stores';
import { useSaveConflict } from '../composables/useSaveConflict';
import SaveRefusalNotice from './SaveRefusalNotice.vue';
import type {
  RecurrenceFrequency,
  RecurringTransaction,
  TransactionType,
  UpdateRecurringRequest,
} from '../types';

const props = defineProps<{
  /** The schedule to edit; `null` keeps the dialog closed. */
  recurringId: string | null;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'saved', record: RecurringTransaction): void;
}>();

const accountsStore = useAccountsStore();
const categoriesStore = useCategoriesStore();

interface Form {
  description: string;
  amount: string;
  transaction_type: TransactionType;
  account_id: string;
  category_id: string;
  payee: string;
  frequency: RecurrenceFrequency;
  next_occurrence: string;
  end_date: string;
}

const original = ref<RecurringTransaction | null>(null);
const form = ref<Form | null>(null);
const loadError = ref<string | null>(null);
const saving = ref(false);

function fill(record: RecurringTransaction) {
  original.value = record;
  form.value = {
    description: record.description,
    amount: record.amount,
    transaction_type: record.transaction_type,
    account_id: record.account_id,
    category_id: record.category_id ?? '',
    payee: record.payee ?? '',
    frequency: record.frequency,
    next_occurrence: record.next_occurrence,
    end_date: record.end_date ?? '',
  };
}

async function fetchRecord(id: string) {
  try {
    return await api.getRecurringById(id);
  } catch {
    return undefined;
  }
}

const conflict = useSaveConflict<RecurringTransaction>({
  label: 'scheduled transaction',
  versionOf: (r) => r.row_version,
  fetchLatest: () => fetchRecord(props.recurringId ?? ''),
  loadIntoForm: fill,
});

watch(
  () => props.recurringId,
  async (id) => {
    original.value = null;
    form.value = null;
    loadError.value = null;
    conflict.clear();
    if (!id) return;
    if (!accountsStore.accounts.length) void accountsStore.fetchAccounts();
    if (!categoriesStore.categories.length) void categoriesStore.fetchCategories();
    const record = await fetchRecord(id);
    if (props.recurringId !== id) return; // switched while loading
    if (!record) {
      loadError.value = 'This scheduled transaction could not be found. It may have been deleted.';
      return;
    }
    fill(record);
    conflict.begin(record);
  },
  { immediate: true }
);

/** Income categories for income, expense categories for expenses. */
const categoryChoices = computed(() => {
  const type = form.value?.transaction_type;
  const list = type === 'income' ? categoriesStore.incomeCategories : categoriesStore.expenseCategories;
  const current = form.value?.category_id;
  // Keep the current category listed even if it is of the other kind, so
  // opening the form never silently changes it.
  const extra = current && !list.some((c) => c.id === current) ? categoriesStore.categoriesById[current] : undefined;
  return [...(extra ? [extra] : []), ...list].sort((a, b) => a.name.localeCompare(b.name));
});

const nextChanged = computed(
  () => !!form.value && !!original.value && form.value.next_occurrence !== original.value.next_occurrence
);

const canSave = computed(() => {
  const f = form.value;
  return !!f && !saving.value && f.description.trim() !== '' && Number(f.amount) > 0 && !!f.account_id && !!f.next_occurrence;
});

async function save() {
  const f = form.value;
  const before = original.value;
  if (!f || !before || !canSave.value) return;

  const request: UpdateRecurringRequest = {
    id: before.id,
    description: f.description.trim(),
    amount: Number(f.amount).toFixed(2),
    transaction_type: f.transaction_type,
    account_id: f.account_id,
    category_id: f.category_id || null,
    payee: f.payee.trim() || null,
    frequency: f.frequency,
    next_occurrence: f.next_occurrence,
    end_date: f.end_date || null,
  };
  // Monthly schedules fall on a fixed day of the month. Moving the date, or
  // switching to monthly, moves that day too, so the ones after follow. Left
  // alone otherwise: a bill on the 31st shown as the 30th in a short month
  // must not be re-pinned to the 30th just because its amount was edited.
  if (nextChanged.value || f.frequency !== before.frequency) {
    request.day_of_month = f.frequency === 'monthly' ? Number(f.next_occurrence.slice(8, 10)) : null;
  }

  saving.value = true;
  try {
    const saved = await api.updateRecurring(request, conflict.version.value);
    emit('saved', saved);
    emit('close');
  } catch (e) {
    conflict.refused(e);
  } finally {
    saving.value = false;
  }
}

const inputClass =
  'w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white';
const labelClass = 'block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1';
</script>

<template>
  <div
    v-if="recurringId"
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
    @click.self="emit('close')"
  >
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-lg w-full max-h-[90vh] flex flex-col">
      <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Edit Scheduled Transaction</h2>
        <button
          class="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
          aria-label="Close"
          @click="emit('close')"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <div v-if="loadError" class="p-4 text-sm text-red-700 dark:text-red-300">{{ loadError }}</div>
      <div v-else-if="!form" class="p-8 flex justify-center">
        <div class="animate-spin rounded-full h-6 w-6 border-b-2 border-blue-600"></div>
      </div>

      <form v-else class="p-4 space-y-4 overflow-y-auto" @submit.prevent="save">
        <SaveRefusalNotice
          :sentence="conflict.sentence.value"
          :kind="conflict.kind.value"
          :note="conflict.note.value"
          :reloading="conflict.reloading.value"
          @load-latest="conflict.loadLatest()"
          @keep-mine="conflict.keepMine()"
        />

        <div>
          <label :class="labelClass" for="rec-description">Description</label>
          <input id="rec-description" v-model="form.description" type="text" :class="inputClass" />
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div>
            <label :class="labelClass" for="rec-amount">Amount</label>
            <input id="rec-amount" v-model="form.amount" type="number" step="0.01" min="0.01" :class="inputClass" />
          </div>
          <div>
            <label :class="labelClass" for="rec-type">Type</label>
            <select id="rec-type" v-model="form.transaction_type" :class="inputClass">
              <option value="expense">Expense</option>
              <option value="income">Income</option>
            </select>
          </div>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div>
            <label :class="labelClass" for="rec-frequency">Frequency</label>
            <select id="rec-frequency" v-model="form.frequency" :class="inputClass">
              <option value="daily">Daily</option>
              <option value="weekly">Weekly</option>
              <option value="biweekly">Every 2 weeks</option>
              <option value="monthly">Monthly</option>
              <option value="quarterly">Quarterly</option>
              <option value="yearly">Yearly</option>
            </select>
          </div>
          <div>
            <label :class="labelClass" for="rec-next">Next due</label>
            <input id="rec-next" v-model="form.next_occurrence" type="date" :class="inputClass" />
          </div>
        </div>
        <p v-if="nextChanged" class="-mt-2 text-xs text-gray-500 dark:text-gray-400">
          The ones after this follow on from the new date.
        </p>

        <div>
          <label :class="labelClass" for="rec-account">Account</label>
          <select id="rec-account" v-model="form.account_id" :class="inputClass">
            <option v-for="account in accountsStore.accounts" :key="account.id" :value="account.id">
              {{ account.name }}
            </option>
          </select>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div>
            <label :class="labelClass" for="rec-category">Category</label>
            <select id="rec-category" v-model="form.category_id" :class="inputClass">
              <option value="">Uncategorized</option>
              <option v-for="c in categoryChoices" :key="c.id" :value="c.id">{{ c.name }}</option>
            </select>
          </div>
          <div>
            <label :class="labelClass" for="rec-payee">Payee <span class="font-normal text-gray-400">(optional)</span></label>
            <input id="rec-payee" v-model="form.payee" type="text" :class="inputClass" />
          </div>
        </div>

        <div>
          <label :class="labelClass" for="rec-end">Ends <span class="font-normal text-gray-400">(optional)</span></label>
          <div class="flex gap-2">
            <input id="rec-end" v-model="form.end_date" type="date" :class="inputClass" />
            <button
              v-if="form.end_date"
              type="button"
              class="px-3 py-2 text-sm rounded-lg text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 whitespace-nowrap"
              @click="form.end_date = ''"
            >
              No end date
            </button>
          </div>
        </div>
      </form>

      <div class="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-3">
        <button
          type="button"
          class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
          @click="emit('close')"
        >
          Cancel
        </button>
        <button
          type="button"
          :disabled="!canSave"
          class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50"
          @click="save"
        >
          {{ saving ? 'Saving…' : 'Save Changes' }}
        </button>
      </div>
    </div>
  </div>
</template>
