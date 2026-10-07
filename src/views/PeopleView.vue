<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue';
import * as api from '../services/api';
import type { AccessLevel, Person } from '../services/api';
import { useMultiUserStore } from '../stores/multiuser';

const store = useMultiUserStore();

const AREAS = [
  { key: 'money', label: 'Money', covers: 'Accounts, transactions, transfers, the bill calendar' },
  { key: 'planning', label: 'Planning', covers: 'Budgets, savings goals, recurring bills' },
  { key: 'structure', label: 'Structure', covers: 'Categories and categorisation rules' },
  { key: 'reports', label: 'Reports', covers: 'Spending charts, cash flow, trends' },
  { key: 'admin', label: 'Admin', covers: 'People, paired computers, maintenance' },
] as const;

type Grants = Record<string, AccessLevel>;

/**
 * Starting points, so nobody has to understand five areas to add a partner.
 * "Administrator" is the owner flag, which always means full access.
 */
const PRESETS: { key: string; label: string; explain: string; isOwner: boolean; grants: Grants }[] = [
  {
    key: 'administrator',
    label: 'Administrator',
    explain: 'Everything, including adding and removing people.',
    isOwner: true,
    grants: {},
  },
  {
    key: 'full',
    label: 'Full access',
    explain: 'Everything except managing people and computers.',
    isOwner: false,
    grants: { money: 'write', planning: 'write', structure: 'write', reports: 'read' },
  },
  {
    key: 'spending',
    label: 'Records spending',
    explain: 'Adds and edits transactions; can see budgets, categories and reports.',
    isOwner: false,
    grants: { money: 'write', planning: 'read', structure: 'read', reports: 'read' },
  },
  {
    key: 'view',
    label: 'View only',
    explain: 'Sees everything except people and computers; changes nothing.',
    isOwner: false,
    grants: { money: 'read', planning: 'read', structure: 'read', reports: 'read' },
  },
];

const people = ref<Person[]>([]);
const loading = ref(true);
const busy = ref(false);
const notice = ref<string | null>(null);
const success = ref<string | null>(null);
const refused = ref<string | null>(null);

function message(e: unknown) {
  return e instanceof Error ? e.message : String(e);
}

async function run(done: string, fn: () => Promise<unknown>) {
  busy.value = true;
  notice.value = null;
  success.value = null;
  try {
    await fn();
    success.value = done;
    await load();
    return true;
  } catch (e) {
    notice.value = message(e);
    return false;
  } finally {
    busy.value = false;
  }
}

async function load() {
  try {
    people.value = await api.listUsers();
    refused.value = null;
  } catch (e) {
    // A person without the Admin grant is refused by the host, by name.
    refused.value = message(e);
    people.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(async () => {
  await store.refreshStatus().catch(() => {});
  await load();
});
// Another administrator changed someone's access somewhere else.
watch(() => store.peopleTick, load);

// ------------------------------------------------------------ access editing

function presetFor(isOwner: boolean, grants: Grants): string {
  if (isOwner) return 'administrator';
  const same = (a: Grants, b: Grants) =>
    AREAS.every((area) => (a[area.key] ?? 'none') === (b[area.key] ?? 'none'));
  return PRESETS.find((p) => !p.isOwner && same(p.grants, grants))?.key ?? 'custom';
}

/** Every area present, so each row of radio buttons always shows a choice. */
function fullGrants(grants: Grants): Grants {
  const out: Grants = {};
  for (const area of AREAS) out[area.key] = grants[area.key] ?? 'none';
  return out;
}

function cleanGrants(grants: Grants): Grants {
  const out: Grants = {};
  for (const area of AREAS) {
    const level = grants[area.key] ?? 'none';
    if (level !== 'none') out[area.key] = level;
  }
  return out;
}

function summary(person: Person) {
  if (person.is_owner) return 'Administrator';
  const preset = PRESETS.find((p) => p.key === presetFor(false, person.grants));
  if (preset) return preset.label;
  const parts = AREAS.filter((a) => person.grants[a.key]).map(
    (a) => `${a.label} ${person.grants[a.key] === 'write' ? '(edit)' : '(view)'}`
  );
  return parts.length ? parts.join(', ') : 'No access';
}

/** Shared by the add form and the edit panel. */
function accessEditor(initialOwner = false, initialGrants: Grants = PRESETS[2].grants) {
  const editor = reactive({
    preset: presetFor(initialOwner, initialGrants),
    grants: fullGrants(initialGrants),
  });
  return editor;
}

function choosePreset(editor: { preset: string; grants: Grants }, key: string) {
  editor.preset = key;
  const preset = PRESETS.find((p) => p.key === key);
  if (preset) editor.grants = fullGrants(preset.grants);
}

function isOwnerOf(editor: { preset: string }) {
  return editor.preset === 'administrator';
}

// ---------------------------------------------------------------- adding

const adding = ref(false);
const draft = reactive({ displayName: '', login: '', password: '' });
const draftAccess = accessEditor();

function startAdding() {
  adding.value = true;
  draft.displayName = '';
  draft.login = '';
  draft.password = '';
  Object.assign(draftAccess, accessEditor());
}

async function addPerson() {
  const ok = await run(`${draft.displayName} can now sign in.`, () =>
    api.createPerson({
      login: draft.login.trim(),
      displayName: draft.displayName.trim(),
      password: draft.password,
      isOwner: isOwnerOf(draftAccess),
      grants: cleanGrants(draftAccess.grants),
    })
  );
  if (ok) adding.value = false;
}

// --------------------------------------------------------------- editing

const editingId = ref<string | null>(null);
const editAccess = accessEditor();

function startEditing(person: Person) {
  resettingId.value = null;
  editingId.value = person.id;
  Object.assign(editAccess, accessEditor(person.is_owner, person.grants));
}

async function saveAccess(person: Person) {
  const ok = await run(`${person.display_name}'s access was updated.`, () =>
    api.setUserGrants(person.id, cleanGrants(editAccess.grants), isOwnerOf(editAccess))
  );
  if (ok) editingId.value = null;
}

const resettingId = ref<string | null>(null);
const newPassword = ref('');

function startResetting(person: Person) {
  editingId.value = null;
  resettingId.value = person.id;
  newPassword.value = '';
}

async function resetPassword(person: Person) {
  const ok = await run(`${person.display_name}'s password was changed.`, () =>
    api.changeUserPassword(person.id, newPassword.value)
  );
  if (ok) resettingId.value = null;
}

function toggleActive(person: Person) {
  return run(
    person.is_active
      ? `${person.display_name} can no longer sign in.`
      : `${person.display_name} can sign in again.`,
    () => api.setUserActive(person.id, !person.is_active)
  );
}

function remove(person: Person) {
  if (
    !window.confirm(
      `Remove ${person.display_name}? They will be signed out at once and cannot sign in again. ` +
        'What they recorded stays in the budget.'
    )
  )
    return;
  return run(`${person.display_name} was removed.`, () => api.deleteUser(person.id));
}

const passwordRules = 'At least 8 characters, with an uppercase letter, a lowercase letter and a number.';
const sorted = computed(() =>
  [...people.value].sort(
    (a, b) =>
      Number(b.is_active) - Number(a.is_active) ||
      Number(b.is_owner) - Number(a.is_owner) ||
      a.display_name.localeCompare(b.display_name)
  )
);
</script>

<template>
  <div class="p-8 max-w-4xl">
    <header class="mb-6 flex items-start justify-between gap-4">
      <div>
        <h1 class="text-2xl font-bold text-gray-900 dark:text-white">People</h1>
        <p class="text-sm text-gray-600 dark:text-gray-400 mt-1">
          Who can sign in to this budget from another computer, and what each of them can reach.
          Whoever is at the computer hosting the budget always has full access.
        </p>
      </div>
      <button
        v-if="!refused && !adding"
        :disabled="busy"
        class="shrink-0 px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
        @click="startAdding"
      >
        Add a person
      </button>
    </header>

    <div
      v-if="notice"
      role="alert"
      class="mb-4 px-4 py-3 rounded-lg bg-amber-50 dark:bg-amber-900/30 text-amber-800 dark:text-amber-300 text-sm"
    >
      {{ notice }}
    </div>
    <div
      v-if="success"
      role="status"
      class="mb-4 px-4 py-3 rounded-lg bg-green-50 dark:bg-green-900/30 text-green-800 dark:text-green-300 text-sm"
    >
      {{ success }}
    </div>

    <div
      v-if="refused"
      class="p-5 rounded-xl border border-gray-200 dark:border-gray-700 text-sm text-gray-700 dark:text-gray-300"
    >
      {{ refused }}
    </div>

    <template v-else>
      <!-- Add a person -->
      <section
        v-if="adding"
        class="mb-6 p-5 rounded-xl border border-blue-200 dark:border-blue-800 bg-white dark:bg-gray-800"
      >
        <h2 class="font-semibold text-gray-900 dark:text-white mb-4">Add a person</h2>
        <div class="grid gap-3 md:grid-cols-2">
          <label class="text-sm text-gray-700 dark:text-gray-300">
            Name
            <input
              v-model="draft.displayName"
              placeholder="Alex"
              class="mt-1 w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-800"
            />
          </label>
          <label class="text-sm text-gray-700 dark:text-gray-300">
            Login
            <input
              v-model="draft.login"
              placeholder="alex"
              autocapitalize="off"
              class="mt-1 w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-800"
            />
          </label>
          <label class="text-sm text-gray-700 dark:text-gray-300 md:col-span-2">
            Password
            <input
              v-model="draft.password"
              type="password"
              class="mt-1 w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-800"
            />
            <span class="text-xs text-gray-500 dark:text-gray-400">{{ passwordRules }}</span>
          </label>
        </div>

        <fieldset class="mt-5">
          <legend class="text-sm font-medium text-gray-900 dark:text-white mb-2">What can they do?</legend>
          <div class="grid gap-2 md:grid-cols-2">
            <label
              v-for="p in PRESETS"
              :key="p.key"
              class="flex gap-2 p-3 rounded-lg border cursor-pointer"
              :class="
                draftAccess.preset === p.key
                  ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/30'
                  : 'border-gray-200 dark:border-gray-700'
              "
            >
              <input
                type="radio"
                :checked="draftAccess.preset === p.key"
                @change="choosePreset(draftAccess, p.key)"
              />
              <span>
                <span class="block text-sm font-medium text-gray-900 dark:text-white">{{ p.label }}</span>
                <span class="block text-xs text-gray-600 dark:text-gray-400">{{ p.explain }}</span>
              </span>
            </label>
          </div>
          <button
            type="button"
            class="mt-2 text-sm text-blue-600 hover:underline"
            @click="draftAccess.preset = draftAccess.preset === 'custom' ? 'spending' : 'custom'"
          >
            {{ draftAccess.preset === 'custom' ? 'Use a preset instead' : 'Choose area by area' }}
          </button>
          <table v-if="draftAccess.preset === 'custom'" class="mt-3 w-full text-sm">
            <tbody>
              <tr v-for="area in AREAS" :key="area.key" class="border-t border-gray-100 dark:border-gray-700">
                <td class="py-2 pr-3">
                  <span class="font-medium text-gray-900 dark:text-white">{{ area.label }}</span>
                  <span class="block text-xs text-gray-500 dark:text-gray-400">{{ area.covers }}</span>
                </td>
                <td class="py-2 whitespace-nowrap">
                  <label v-for="level in ['none', 'read', 'write']" :key="level" class="mr-3">
                    <input type="radio" :value="level" v-model="draftAccess.grants[area.key]" />
                    {{ level === 'none' ? 'None' : level === 'read' ? 'View' : 'Edit' }}
                  </label>
                </td>
              </tr>
            </tbody>
          </table>
        </fieldset>

        <div class="mt-5 flex gap-2">
          <button
            :disabled="busy || !draft.displayName.trim() || !draft.login.trim() || !draft.password"
            class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
            @click="addPerson"
          >
            {{ busy ? 'Adding…' : 'Add' }}
          </button>
          <button
            :disabled="busy"
            class="px-4 py-2 rounded-lg text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
            @click="adding = false"
          >
            Cancel
          </button>
        </div>
      </section>

      <!-- Nobody yet -->
      <div
        v-if="!loading && !people.length && !adding"
        class="p-8 rounded-xl border border-dashed border-gray-300 dark:border-gray-600 text-center"
      >
        <p class="text-gray-900 dark:text-white font-medium">Nobody can sign in from another computer yet.</p>
        <p class="text-sm text-gray-600 dark:text-gray-400 mt-1">
          Add the people who will use this budget, then start hosting from the Sharing screen.
        </p>
      </div>

      <!-- Everyone -->
      <ul v-else class="space-y-3">
        <li
          v-for="person in sorted"
          :key="person.id"
          class="p-4 rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800"
          :class="person.is_active ? '' : 'opacity-70'"
        >
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div>
              <p class="font-medium text-gray-900 dark:text-white">
                {{ person.display_name }}
                <span class="text-sm font-normal text-gray-500 dark:text-gray-400">({{ person.login }})</span>
                <span
                  v-if="!person.is_active"
                  class="ml-2 text-xs px-2 py-0.5 rounded-full bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300"
                  >Deactivated</span
                >
              </p>
              <p class="text-sm text-gray-600 dark:text-gray-400">{{ summary(person) }}</p>
            </div>
            <div class="flex flex-wrap gap-3 text-sm">
              <button :disabled="busy" class="text-blue-600 hover:underline" @click="startEditing(person)">
                Change access
              </button>
              <button :disabled="busy" class="text-blue-600 hover:underline" @click="startResetting(person)">
                Set password
              </button>
              <button :disabled="busy" class="text-gray-700 dark:text-gray-300 hover:underline" @click="toggleActive(person)">
                {{ person.is_active ? 'Deactivate' : 'Reactivate' }}
              </button>
              <button :disabled="busy" class="text-red-600 hover:underline" @click="remove(person)">Remove</button>
            </div>
          </div>

          <!-- Change access -->
          <div v-if="editingId === person.id" class="mt-4 pt-4 border-t border-gray-100 dark:border-gray-700">
            <div class="grid gap-2 md:grid-cols-2">
              <label
                v-for="p in PRESETS"
                :key="p.key"
                class="flex gap-2 p-3 rounded-lg border cursor-pointer"
                :class="
                  editAccess.preset === p.key
                    ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/30'
                    : 'border-gray-200 dark:border-gray-700'
                "
              >
                <input type="radio" :checked="editAccess.preset === p.key" @change="choosePreset(editAccess, p.key)" />
                <span>
                  <span class="block text-sm font-medium text-gray-900 dark:text-white">{{ p.label }}</span>
                  <span class="block text-xs text-gray-600 dark:text-gray-400">{{ p.explain }}</span>
                </span>
              </label>
            </div>
            <button
              type="button"
              class="mt-2 text-sm text-blue-600 hover:underline"
              @click="editAccess.preset = editAccess.preset === 'custom' ? presetFor(false, editAccess.grants) : 'custom'"
            >
              {{ editAccess.preset === 'custom' ? 'Use a preset instead' : 'Choose area by area' }}
            </button>
            <table v-if="editAccess.preset === 'custom'" class="mt-3 w-full text-sm">
              <tbody>
                <tr v-for="area in AREAS" :key="area.key" class="border-t border-gray-100 dark:border-gray-700">
                  <td class="py-2 pr-3">
                    <span class="font-medium text-gray-900 dark:text-white">{{ area.label }}</span>
                    <span class="block text-xs text-gray-500 dark:text-gray-400">{{ area.covers }}</span>
                  </td>
                  <td class="py-2 whitespace-nowrap">
                    <label v-for="level in ['none', 'read', 'write']" :key="level" class="mr-3">
                      <input type="radio" :value="level" v-model="editAccess.grants[area.key]" />
                      {{ level === 'none' ? 'None' : level === 'read' ? 'View' : 'Edit' }}
                    </label>
                  </td>
                </tr>
              </tbody>
            </table>
            <p class="mt-3 text-xs text-gray-500 dark:text-gray-400">
              Takes effect at once, even if {{ person.display_name }} is signed in right now.
            </p>
            <div class="mt-3 flex gap-2">
              <button
                :disabled="busy"
                class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
                @click="saveAccess(person)"
              >
                Save
              </button>
              <button
                :disabled="busy"
                class="px-4 py-2 rounded-lg text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
                @click="editingId = null"
              >
                Cancel
              </button>
            </div>
          </div>

          <!-- Set password -->
          <div v-if="resettingId === person.id" class="mt-4 pt-4 border-t border-gray-100 dark:border-gray-700">
            <label class="text-sm text-gray-700 dark:text-gray-300">
              New password for {{ person.display_name }}
              <input
                v-model="newPassword"
                type="password"
                class="mt-1 w-full max-w-sm px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-800"
              />
              <span class="block text-xs text-gray-500 dark:text-gray-400">{{ passwordRules }}</span>
            </label>
            <div class="mt-3 flex gap-2">
              <button
                :disabled="busy || !newPassword"
                class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
                @click="resetPassword(person)"
              >
                Set password
              </button>
              <button
                :disabled="busy"
                class="px-4 py-2 rounded-lg text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
                @click="resettingId = null"
              >
                Cancel
              </button>
            </div>
          </div>
        </li>
      </ul>
    </template>
  </div>
</template>
