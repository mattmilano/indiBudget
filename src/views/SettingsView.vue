<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import { save, open } from '@tauri-apps/plugin-dialog';
import * as api from '../services/api';
import { useTheme, type ThemeMode } from '../composables/useTheme';
import type { EncryptionStatus, BackupMetadata } from '../types';

const { currentTheme, setTheme } = useTheme();

const settings = ref({
  currency: 'USD',
  dateFormat: 'MM/DD/YYYY',
  theme: currentTheme.value as string,
  notifications: {
    upcomingBills: true,
    budgetAlerts: true,
    goalProgress: true,
    reminderDays: 3,
    showAmount: true,
  },
});

// Watch for theme changes and apply them
watch(() => settings.value.theme, (newTheme) => {
  setTheme(newTheme as ThemeMode);
});

const notificationStatus = ref('');
const isCheckingNotifications = ref(false);

// Encryption state
const encryptionStatus = ref<EncryptionStatus>({ enabled: false, unlocked: false });
const encryptionPassword = ref('');
const encryptionConfirmPassword = ref('');
const encryptionOldPassword = ref('');
const encryptionNewPassword = ref('');
const encryptionMessage = ref('');
const encryptionError = ref('');
const showChangePassword = ref(false);
const isEncryptionLoading = ref(false);

// Backup state
const backupMessage = ref('');
const backupError = ref('');
const isBackupLoading = ref(false);
const lastBackupInfo = ref<BackupMetadata | null>(null);

// Debug info
const databasePath = ref('');
const transactionCount = ref(0);

onMounted(async () => {
  // Load saved settings from localStorage
  const saved = localStorage.getItem('indibudget-settings');
  if (saved) {
    try {
      const parsed = JSON.parse(saved);
      settings.value = { ...settings.value, ...parsed };
    } catch (e) {
      console.error('Failed to load settings:', e);
    }
  }

  // Sync theme from the composable (it handles its own storage)
  settings.value.theme = currentTheme.value;

  // Load encryption status
  try {
    encryptionStatus.value = await api.getEncryptionStatus();
  } catch (e) {
    console.error('Failed to get encryption status:', e);
  }

  // Load debug info
  try {
    databasePath.value = await api.getDatabasePath();
    transactionCount.value = await api.getTransactionCount();
  } catch (e) {
    console.error('Failed to get debug info:', e);
  }
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
  notificationStatus.value = 'Settings saved successfully!';
  setTimeout(() => { notificationStatus.value = ''; }, 3000);
}

async function testNotification() {
  try {
    await api.sendBillNotification(
      'Test Notification',
      'This is a test notification from indiBudget. Your notifications are working!'
    );
    notificationStatus.value = 'Test notification sent!';
    setTimeout(() => { notificationStatus.value = ''; }, 3000);
  } catch (e) {
    console.error('Failed to send test notification:', e);
    notificationStatus.value = 'Failed to send notification. Check permissions.';
  }
}

async function checkNotificationsNow() {
  if (!settings.value.notifications.upcomingBills) {
    notificationStatus.value = 'Bill notifications are disabled.';
    setTimeout(() => { notificationStatus.value = ''; }, 3000);
    return;
  }

  isCheckingNotifications.value = true;
  try {
    const sent = await api.checkAndSendNotifications(
      settings.value.notifications.reminderDays,
      settings.value.notifications.showAmount
    );
    if (sent > 0) {
      notificationStatus.value = `Sent ${sent} bill reminder(s)!`;
    } else {
      notificationStatus.value = 'No upcoming bills within reminder period.';
    }
    setTimeout(() => { notificationStatus.value = ''; }, 3000);
  } catch (e) {
    console.error('Failed to check notifications:', e);
    notificationStatus.value = 'Failed to check notifications.';
  } finally {
    isCheckingNotifications.value = false;
  }
}

async function exportData() {
  backupMessage.value = '';
  backupError.value = '';

  try {
    const defaultPath = await api.getDefaultBackupPath();
    const path = await save({
      defaultPath,
      filters: [{ name: 'JSON Backup', extensions: ['json'] }],
      title: 'Export Backup',
    });

    if (!path) return;

    isBackupLoading.value = true;
    const metadata = await api.exportBackup(path);
    lastBackupInfo.value = metadata;
    backupMessage.value = `Backup exported successfully! ${metadata.account_count} accounts, ${metadata.transaction_count} transactions.`;
    setTimeout(() => { backupMessage.value = ''; }, 5000);
  } catch (e) {
    console.error('Failed to export backup:', e);
    backupError.value = 'Failed to export backup. Please try again.';
  } finally {
    isBackupLoading.value = false;
  }
}

async function importData() {
  backupMessage.value = '';
  backupError.value = '';

  try {
    const path = await open({
      multiple: false,
      filters: [{ name: 'JSON Backup', extensions: ['json'] }],
      title: 'Import Backup',
    });

    if (!path || typeof path !== 'string') return;

    // First, get info about the backup
    const info = await api.getBackupInfo(path);

    // Confirm import
    const confirmed = confirm(
      `This backup contains:\n` +
      `- ${info.account_count} accounts\n` +
      `- ${info.transaction_count} transactions\n` +
      `- ${info.category_count} categories\n\n` +
      `Created: ${new Date(info.created_at).toLocaleString()}\n\n` +
      `Do you want to import this backup?`
    );

    if (!confirmed) return;

    isBackupLoading.value = true;
    const metadata = await api.importBackup(path);
    lastBackupInfo.value = metadata;
    backupMessage.value = `Backup imported successfully! ${metadata.account_count} accounts, ${metadata.transaction_count} transactions restored.`;
    setTimeout(() => { backupMessage.value = ''; }, 5000);
  } catch (e) {
    console.error('Failed to import backup:', e);
    backupError.value = 'Failed to import backup. Please check the file format.';
  } finally {
    isBackupLoading.value = false;
  }
}

// Encryption functions
function clearEncryptionMessages() {
  encryptionMessage.value = '';
  encryptionError.value = '';
}

async function enableEncryption() {
  clearEncryptionMessages();

  if (encryptionPassword.value.length < 8) {
    encryptionError.value = 'Password must be at least 8 characters.';
    return;
  }

  if (encryptionPassword.value !== encryptionConfirmPassword.value) {
    encryptionError.value = 'Passwords do not match.';
    return;
  }

  isEncryptionLoading.value = true;
  try {
    await api.enableEncryption(encryptionPassword.value);
    encryptionStatus.value = await api.getEncryptionStatus();
    encryptionMessage.value = 'Encryption enabled successfully!';
    encryptionPassword.value = '';
    encryptionConfirmPassword.value = '';
  } catch (e) {
    encryptionError.value = 'Failed to enable encryption. Please try again.';
    console.error('Failed to enable encryption:', e);
  } finally {
    isEncryptionLoading.value = false;
  }
}

async function disableEncryption() {
  clearEncryptionMessages();

  if (!encryptionPassword.value) {
    encryptionError.value = 'Please enter your password to disable encryption.';
    return;
  }

  isEncryptionLoading.value = true;
  try {
    await api.disableEncryption(encryptionPassword.value);
    encryptionStatus.value = await api.getEncryptionStatus();
    encryptionMessage.value = 'Encryption disabled successfully.';
    encryptionPassword.value = '';
  } catch (e) {
    encryptionError.value = 'Invalid password. Please try again.';
    console.error('Failed to disable encryption:', e);
  } finally {
    isEncryptionLoading.value = false;
  }
}

async function unlockEncryption() {
  clearEncryptionMessages();

  if (!encryptionPassword.value) {
    encryptionError.value = 'Please enter your password.';
    return;
  }

  isEncryptionLoading.value = true;
  try {
    await api.unlockEncryption(encryptionPassword.value);
    encryptionStatus.value = await api.getEncryptionStatus();
    encryptionMessage.value = 'Encryption unlocked!';
    encryptionPassword.value = '';
  } catch (e) {
    encryptionError.value = 'Invalid password. Please try again.';
    console.error('Failed to unlock encryption:', e);
  } finally {
    isEncryptionLoading.value = false;
  }
}

async function lockEncryption() {
  clearEncryptionMessages();

  try {
    await api.lockEncryption();
    encryptionStatus.value = await api.getEncryptionStatus();
    encryptionMessage.value = 'Encryption locked.';
  } catch (e) {
    encryptionError.value = 'Failed to lock encryption.';
    console.error('Failed to lock encryption:', e);
  }
}

async function changeEncryptionPassword() {
  clearEncryptionMessages();

  if (encryptionNewPassword.value.length < 8) {
    encryptionError.value = 'New password must be at least 8 characters.';
    return;
  }

  isEncryptionLoading.value = true;
  try {
    await api.changeEncryptionPassword(encryptionOldPassword.value, encryptionNewPassword.value);
    encryptionMessage.value = 'Password changed successfully!';
    encryptionOldPassword.value = '';
    encryptionNewPassword.value = '';
    showChangePassword.value = false;
  } catch (e) {
    encryptionError.value = 'Failed to change password. Please check your current password.';
    console.error('Failed to change encryption password:', e);
  } finally {
    isEncryptionLoading.value = false;
  }
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
          <!-- Status message -->
          <div
            v-if="notificationStatus"
            class="px-4 py-2 rounded-lg bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-400 text-sm"
          >
            {{ notificationStatus }}
          </div>

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
          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium text-gray-900 dark:text-white">Show Amounts</p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Include payment amounts in notifications</p>
            </div>
            <input
              v-model="settings.notifications.showAmount"
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
          <div class="flex gap-3 pt-2">
            <button
              @click="testNotification"
              class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
            >
              Send Test
            </button>
            <button
              @click="checkNotificationsNow"
              :disabled="isCheckingNotifications"
              class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50"
            >
              {{ isCheckingNotifications ? 'Checking...' : 'Check Now' }}
            </button>
          </div>
        </div>
      </div>

      <!-- Security -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Security</h2>
        </div>
        <div class="p-4 space-y-4">
          <!-- Status messages -->
          <div
            v-if="encryptionMessage"
            class="px-4 py-2 rounded-lg bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400 text-sm"
          >
            {{ encryptionMessage }}
          </div>
          <div
            v-if="encryptionError"
            class="px-4 py-2 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 text-sm"
          >
            {{ encryptionError }}
          </div>

          <!-- Encryption status indicator -->
          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium text-gray-900 dark:text-white">Data Encryption</p>
              <p class="text-sm text-gray-500 dark:text-gray-400">
                Encrypt sensitive data at rest using AES-256 encryption
              </p>
            </div>
            <span
              :class="[
                'px-3 py-1 rounded-full text-sm font-medium',
                encryptionStatus.enabled
                  ? encryptionStatus.unlocked
                    ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400'
                    : 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-400'
                  : 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400'
              ]"
            >
              {{ encryptionStatus.enabled ? (encryptionStatus.unlocked ? 'Unlocked' : 'Locked') : 'Disabled' }}
            </span>
          </div>

          <!-- Not enabled: show enable form -->
          <div v-if="!encryptionStatus.enabled" class="space-y-3 pt-2">
            <p class="text-sm text-gray-600 dark:text-gray-400">
              Enable encryption to protect your financial data with a password.
              You will need this password to access your data.
            </p>
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Password</label>
              <input
                v-model="encryptionPassword"
                type="password"
                placeholder="Enter a strong password (min 8 chars)"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
            </div>
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Confirm Password</label>
              <input
                v-model="encryptionConfirmPassword"
                type="password"
                placeholder="Confirm your password"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
            </div>
            <button
              @click="enableEncryption"
              :disabled="isEncryptionLoading"
              class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50"
            >
              {{ isEncryptionLoading ? 'Enabling...' : 'Enable Encryption' }}
            </button>
          </div>

          <!-- Enabled but locked: show unlock form -->
          <div v-else-if="!encryptionStatus.unlocked" class="space-y-3 pt-2">
            <p class="text-sm text-gray-600 dark:text-gray-400">
              Enter your password to unlock encrypted data.
            </p>
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Password</label>
              <input
                v-model="encryptionPassword"
                type="password"
                placeholder="Enter your encryption password"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
            </div>
            <button
              @click="unlockEncryption"
              :disabled="isEncryptionLoading"
              class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50"
            >
              {{ isEncryptionLoading ? 'Unlocking...' : 'Unlock' }}
            </button>
          </div>

          <!-- Enabled and unlocked: show management options -->
          <div v-else class="space-y-4 pt-2">
            <div class="flex gap-3">
              <button
                @click="lockEncryption"
                class="px-4 py-2 bg-yellow-600 text-white rounded-lg hover:bg-yellow-700 transition-colors"
              >
                Lock Now
              </button>
              <button
                @click="showChangePassword = !showChangePassword"
                class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
              >
                Change Password
              </button>
            </div>

            <!-- Change password form -->
            <div v-if="showChangePassword" class="space-y-3 p-4 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Current Password</label>
                <input
                  v-model="encryptionOldPassword"
                  type="password"
                  placeholder="Enter current password"
                  class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                />
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">New Password</label>
                <input
                  v-model="encryptionNewPassword"
                  type="password"
                  placeholder="Enter new password (min 8 chars)"
                  class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                />
              </div>
              <button
                @click="changeEncryptionPassword"
                :disabled="isEncryptionLoading"
                class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50"
              >
                {{ isEncryptionLoading ? 'Changing...' : 'Change Password' }}
              </button>
            </div>

            <!-- Disable encryption -->
            <div class="pt-4 border-t border-gray-200 dark:border-gray-600">
              <p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
                To disable encryption, enter your password:
              </p>
              <div class="flex gap-3">
                <input
                  v-model="encryptionPassword"
                  type="password"
                  placeholder="Enter password to disable"
                  class="flex-1 px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                />
                <button
                  @click="disableEncryption"
                  :disabled="isEncryptionLoading"
                  class="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors disabled:opacity-50"
                >
                  {{ isEncryptionLoading ? 'Disabling...' : 'Disable' }}
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Data Management -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Data Management</h2>
        </div>
        <div class="p-4 space-y-4">
          <!-- Status messages -->
          <div
            v-if="backupMessage"
            class="px-4 py-2 rounded-lg bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400 text-sm"
          >
            {{ backupMessage }}
          </div>
          <div
            v-if="backupError"
            class="px-4 py-2 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 text-sm"
          >
            {{ backupError }}
          </div>

          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium text-gray-900 dark:text-white">Export Data</p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Download all your data as a backup</p>
            </div>
            <button
              @click="exportData"
              :disabled="isBackupLoading"
              class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50"
            >
              {{ isBackupLoading ? 'Exporting...' : 'Export' }}
            </button>
          </div>
          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium text-gray-900 dark:text-white">Import Data</p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Restore from a previous backup</p>
            </div>
            <button
              @click="importData"
              :disabled="isBackupLoading"
              class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors disabled:opacity-50"
            >
              {{ isBackupLoading ? 'Importing...' : 'Import' }}
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

          <!-- Debug Info -->
          <div class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
            <p class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Debug Info</p>
            <p class="text-xs text-gray-500 dark:text-gray-400 font-mono break-all">
              Database: {{ databasePath }}
            </p>
            <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
              Transactions in DB: {{ transactionCount }}
            </p>
          </div>
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
