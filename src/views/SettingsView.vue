<script setup lang="ts">
import { ref } from 'vue';

const settings = ref({
  currency: 'USD',
  dateFormat: 'MM/DD/YYYY',
  theme: 'system',
  notifications: {
    upcomingBills: true,
    budgetAlerts: true,
    goalProgress: true,
    reminderDays: 3,
  },
});

const currencies = [
  { value: 'USD', label: 'US Dollar ($)' },
  { value: 'EUR', label: 'Euro (€)' },
  { value: 'GBP', label: 'British Pound (£)' },
  { value: 'CAD', label: 'Canadian Dollar (C$)' },
  { value: 'AUD', label: 'Australian Dollar (A$)' },
  { value: 'JPY', label: 'Japanese Yen (¥)' },
];

const dateFormats = [
  { value: 'MM/DD/YYYY', label: 'MM/DD/YYYY' },
  { value: 'DD/MM/YYYY', label: 'DD/MM/YYYY' },
  { value: 'YYYY-MM-DD', label: 'YYYY-MM-DD' },
];

const themes = [
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
  { value: 'system', label: 'System' },
];

function saveSettings() {
  localStorage.setItem('indibudget-settings', JSON.stringify(settings.value));
  alert('Settings saved successfully!');
}

function exportData() {
  alert('Export functionality coming soon!');
}

function importData() {
  alert('Import functionality coming soon!');
}
</script>

<template>
  <div class="p-6">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white mb-6">Settings</h1>

    <div class="space-y-6">
      <!-- General Settings -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">General</h2>
        </div>
        <div class="p-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Currency</label>
            <select
              v-model="settings.currency"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            >
              <option v-for="currency in currencies" :key="currency.value" :value="currency.value">
                {{ currency.label }}
              </option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Date Format</label>
            <select
              v-model="settings.dateFormat"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            >
              <option v-for="format in dateFormats" :key="format.value" :value="format.value">
                {{ format.label }}
              </option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Theme</label>
            <select
              v-model="settings.theme"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            >
              <option v-for="theme in themes" :key="theme.value" :value="theme.value">
                {{ theme.label }}
              </option>
            </select>
          </div>
        </div>
      </div>

      <!-- Notifications -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Notifications</h2>
        </div>
        <div class="p-4 space-y-4">
          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium text-gray-900 dark:text-white">Upcoming Bills</p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Get reminded about upcoming bill payments</p>
            </div>
            <input
              v-model="settings.notifications.upcomingBills"
              type="checkbox"
              class="w-5 h-5 rounded border-gray-300 dark:border-gray-600"
            />
          </div>
          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium text-gray-900 dark:text-white">Budget Alerts</p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Get notified when approaching budget limits</p>
            </div>
            <input
              v-model="settings.notifications.budgetAlerts"
              type="checkbox"
              class="w-5 h-5 rounded border-gray-300 dark:border-gray-600"
            />
          </div>
          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium text-gray-900 dark:text-white">Goal Progress</p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Receive updates on savings goal progress</p>
            </div>
            <input
              v-model="settings.notifications.goalProgress"
              type="checkbox"
              class="w-5 h-5 rounded border-gray-300 dark:border-gray-600"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Reminder Days Before Due
            </label>
            <input
              v-model="settings.notifications.reminderDays"
              type="number"
              min="1"
              max="30"
              class="w-24 px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>
        </div>
      </div>

      <!-- Data Management -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Data Management</h2>
        </div>
        <div class="p-4 space-y-4">
          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium text-gray-900 dark:text-white">Export Data</p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Download all your data as a backup</p>
            </div>
            <button
              @click="exportData"
              class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
            >
              Export
            </button>
          </div>
          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium text-gray-900 dark:text-white">Import Data</p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Restore from a previous backup</p>
            </div>
            <button
              @click="importData"
              class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
            >
              Import
            </button>
          </div>
        </div>
      </div>

      <!-- About -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">About</h2>
        </div>
        <div class="p-4">
          <p class="text-gray-700 dark:text-gray-300">
            <strong>indiBudget</strong> v0.1.0
          </p>
          <p class="text-sm text-gray-500 dark:text-gray-400 mt-2">
            A personal budget application with calendar-focused expense tracking.
          </p>
          <p class="text-sm text-gray-500 dark:text-gray-400 mt-2">
            Open source software - contributions welcome!
          </p>
        </div>
      </div>

      <!-- Save Button -->
      <div class="flex justify-end">
        <button
          @click="saveSettings"
          class="px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          Save Settings
        </button>
      </div>
    </div>
  </div>
</template>
