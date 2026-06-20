<script setup lang="ts">
import { ref, onMounted, watch, nextTick } from 'vue';
import { save, open } from '@tauri-apps/plugin-dialog';
import * as api from '../services/api';
import * as simplefin from '../services/simplefin';
import { useTheme, type ThemeMode } from '../composables/useTheme';
import { useAccountsStore } from '../stores';
import type { EncryptionStatus, BackupMetadata, SimpleFINConfig, SimpleFINAccount } from '../types';

const { currentTheme, setTheme } = useTheme();
const accountsStore = useAccountsStore();

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
  backup: {
    reminderEnabled: true,
    reminderDays: 7,
  },
});

// Backup reminder state
const lastBackupDate = ref<string | null>(null);
const backupOverdue = ref(false);
const daysSinceBackup = ref(0);

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

function checkBackupStatus() {
  const lastBackup = localStorage.getItem('indibudget-last-backup');
  lastBackupDate.value = lastBackup;

  if (lastBackup && settings.value.backup.reminderEnabled) {
    const lastDate = new Date(lastBackup);
    const now = new Date();
    const diffTime = now.getTime() - lastDate.getTime();
    const diffDays = Math.floor(diffTime / (1000 * 60 * 60 * 24));
    daysSinceBackup.value = diffDays;
    backupOverdue.value = diffDays >= settings.value.backup.reminderDays;
  } else if (!lastBackup) {
    daysSinceBackup.value = -1; // Never backed up
    backupOverdue.value = settings.value.backup.reminderEnabled;
  } else {
    backupOverdue.value = false;
  }
}

function recordBackup() {
  const now = new Date().toISOString();
  localStorage.setItem('indibudget-last-backup', now);
  checkBackupStatus();
}

function formatLastBackupDate() {
  if (!lastBackupDate.value) return 'Never';
  const date = new Date(lastBackupDate.value);
  return date.toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

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

  // Check backup status
  checkBackupStatus();

  // Load encryption status
  try {
    encryptionStatus.value = await api.getEncryptionStatus();
  } catch (e) {
    console.error('Failed to get encryption status:', e);
  }

  // Load accounts for SimpleFIN mapping
  await accountsStore.fetchAccounts();

  // Load SimpleFIN config
  simplefinConfig.value = await simplefin.getConfig();
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
    recordBackup(); // Track the backup date
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

// SimpleFIN Integration State - initialized empty, loaded asynchronously
const simplefinConfig = ref<SimpleFINConfig>({
  accessUrl: null,
  lastSync: null,
  autoSync: false,
  syncInterval: 'manual',
  accountMappings: [],
});
const simplefinAccessUrl = ref('');
const simplefinMessage = ref('');
const simplefinError = ref('');
const simplefinLoading = ref(false);
const simplefinProgress = ref('');
const simplefinAccounts = ref<SimpleFINAccount[]>([]);
const showAccountMapping = ref(false);
const simplefinSyncResult = ref<{ imported: number; skipped: number; errors: string[] } | null>(null);

// Helper to ensure UI updates before heavy operations
function waitForPaint(): Promise<void> {
  return new Promise(resolve => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        resolve();
      });
    });
  });
}

function clearSimplefinMessages() {
  simplefinMessage.value = '';
  simplefinError.value = '';
  simplefinProgress.value = '';
  simplefinSyncResult.value = null;
}

async function testSimplefinConnection() {
  clearSimplefinMessages();

  if (!simplefinAccessUrl.value.trim()) {
    simplefinError.value = 'Please enter your SimpleFIN access URL.';
    return;
  }

  simplefinLoading.value = true;
  simplefinProgress.value = 'Testing connection...';
  await nextTick();
  await waitForPaint();

  try {
    const result = await simplefin.testConnection(simplefinAccessUrl.value);

    if (result.success) {
      simplefinAccounts.value = result.accounts;
      simplefinMessage.value = `Connected successfully! Found ${result.accounts.length} account(s).`;

      // Create default mappings
      const mappings = simplefin.createDefaultMappings(result.accounts);

      // Save the access URL and mappings
      simplefinConfig.value = {
        ...simplefinConfig.value,
        accessUrl: simplefinAccessUrl.value,
        accountMappings: mappings,
      };
      await simplefin.saveConfig(simplefinConfig.value);

      // Show account mapping modal
      showAccountMapping.value = true;
    } else {
      simplefinError.value = result.error || 'Failed to connect to SimpleFIN.';
    }
  } catch (e) {
    simplefinError.value = e instanceof Error ? e.message : 'Failed to connect to SimpleFIN.';
  } finally {
    simplefinLoading.value = false;
    simplefinProgress.value = '';
  }
}

async function syncSimplefin() {
  clearSimplefinMessages();

  if (!simplefinConfig.value.accessUrl) {
    simplefinError.value = 'SimpleFIN is not connected. Please connect first.';
    return;
  }

  // Check if there are any mapped accounts
  const mappedAccounts = simplefinConfig.value.accountMappings.filter(m => m.indibudgetAccountId);
  if (mappedAccounts.length === 0) {
    simplefinError.value = 'No accounts are mapped. Please map at least one account.';
    showAccountMapping.value = true;
    return;
  }

  simplefinLoading.value = true;
  await nextTick();
  await waitForPaint();

  try {
    const result = await simplefin.syncTransactions(
      simplefinConfig.value.accessUrl,
      simplefinConfig.value.accountMappings,
      (msg) => { simplefinProgress.value = msg; }
    );

    simplefinSyncResult.value = {
      imported: result.transactionsImported,
      skipped: result.transactionsSkipped,
      errors: result.errors,
    };

    if (result.errors.length === 0) {
      simplefinMessage.value = `Sync complete! Imported ${result.transactionsImported} transactions, skipped ${result.transactionsSkipped} duplicates.`;
    } else {
      simplefinError.value = `Sync completed with ${result.errors.length} error(s). Imported ${result.transactionsImported}, skipped ${result.transactionsSkipped}.`;
    }

    // Update last sync in config
    simplefinConfig.value = await simplefin.getConfig();
  } catch (e) {
    simplefinError.value = e instanceof Error ? e.message : 'Sync failed.';
  } finally {
    simplefinLoading.value = false;
    simplefinProgress.value = '';
  }
}

async function updateAccountMapping(simplefinAccountId: string, indibudgetAccountId: string | null) {
  const mapping = simplefinConfig.value.accountMappings.find(m => m.simplefinAccountId === simplefinAccountId);
  if (mapping) {
    mapping.indibudgetAccountId = indibudgetAccountId;
    await simplefin.saveConfig(simplefinConfig.value);
  }
}

async function saveSimplefinConfig() {
  await simplefin.saveConfig(simplefinConfig.value);
}

async function disconnectSimplefin() {
  if (confirm('Are you sure you want to disconnect SimpleFIN? Your imported transactions will remain.')) {
    await simplefin.clearConfig();
    simplefinConfig.value = await simplefin.getConfig();
    simplefinAccessUrl.value = '';
    simplefinAccounts.value = [];
    showAccountMapping.value = false;
    clearSimplefinMessages();
    simplefinMessage.value = 'SimpleFIN disconnected.';
  }
}

function formatLastSync(dateStr: string | null): string {
  if (!dateStr) return 'Never';
  const date = new Date(dateStr);
  return date.toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
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
          <!-- Backup reminder warning -->
          <div
            v-if="backupOverdue"
            class="px-4 py-3 rounded-lg bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800"
          >
            <div class="flex items-start gap-3">
              <svg class="w-5 h-5 text-yellow-600 dark:text-yellow-400 mt-0.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
              <div class="flex-1">
                <p class="font-medium text-yellow-800 dark:text-yellow-200">
                  {{ daysSinceBackup < 0 ? 'You have never backed up your data!' : `It's been ${daysSinceBackup} days since your last backup` }}
                </p>
                <p class="text-sm text-yellow-700 dark:text-yellow-300 mt-1">
                  Regular backups protect your financial data from loss. Consider exporting a backup now.
                </p>
              </div>
            </div>
          </div>

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

          <!-- Last backup info -->
          <div class="flex items-center justify-between py-2 px-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
            <span class="text-sm text-gray-600 dark:text-gray-400">Last backup:</span>
            <span class="text-sm font-medium text-gray-900 dark:text-white">{{ formatLastBackupDate() }}</span>
          </div>

          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium text-gray-900 dark:text-white">Export Data</p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Download all your data as a backup</p>
            </div>
            <button
              @click="exportData"
              :disabled="isBackupLoading"
              :class="[
                'px-4 py-2 text-white rounded-lg transition-colors disabled:opacity-50',
                backupOverdue ? 'bg-yellow-600 hover:bg-yellow-700' : 'bg-blue-600 hover:bg-blue-700'
              ]"
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

          <!-- Backup reminder settings -->
          <div class="pt-4 border-t border-gray-200 dark:border-gray-700">
            <h3 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Backup Reminders</h3>
            <div class="space-y-3">
              <div class="flex items-center justify-between">
                <div>
                  <p class="text-sm text-gray-900 dark:text-white">Enable Backup Reminders</p>
                  <p class="text-xs text-gray-500 dark:text-gray-400">Show warnings when backups are overdue</p>
                </div>
                <input
                  v-model="settings.backup.reminderEnabled"
                  type="checkbox"
                  class="w-5 h-5 rounded border-gray-300 dark:border-gray-600"
                  @change="checkBackupStatus()"
                />
              </div>
              <div v-if="settings.backup.reminderEnabled" class="flex items-center gap-3">
                <label class="text-sm text-gray-600 dark:text-gray-400">Remind me if no backup in</label>
                <select
                  v-model="settings.backup.reminderDays"
                  class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                  @change="checkBackupStatus()"
                >
                  <option :value="1">1 day</option>
                  <option :value="3">3 days</option>
                  <option :value="7">7 days</option>
                  <option :value="14">14 days</option>
                  <option :value="30">30 days</option>
                </select>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- SimpleFIN Bank Sync -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <div class="flex items-center gap-3">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Bank Sync</h2>
            <span class="px-2 py-0.5 text-xs font-medium bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-400 rounded">SimpleFIN</span>
          </div>
        </div>
        <div class="p-4 space-y-4">
          <!-- Info box -->
          <div class="px-4 py-3 rounded-lg bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800">
            <p class="text-sm text-blue-800 dark:text-blue-200">
              <strong>SimpleFIN</strong> is a privacy-focused bank aggregation service (~$1.50/month).
              Your bank credentials stay with SimpleFIN - indiBudget only receives transactions.
            </p>
            <a
              href="https://simplefin.org"
              target="_blank"
              class="inline-flex items-center gap-1 mt-2 text-sm text-blue-600 dark:text-blue-400 hover:underline"
            >
              Learn more at simplefin.org
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
              </svg>
            </a>
          </div>

          <!-- Status messages -->
          <div
            v-if="simplefinMessage"
            class="px-4 py-2 rounded-lg bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400 text-sm"
          >
            {{ simplefinMessage }}
          </div>
          <div
            v-if="simplefinError"
            class="px-4 py-2 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 text-sm"
          >
            {{ simplefinError }}
          </div>
          <div
            v-if="simplefinProgress"
            class="px-4 py-2 rounded-lg bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-400 text-sm flex items-center gap-2"
          >
            <svg class="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            {{ simplefinProgress }}
          </div>

          <!-- Not connected: show setup form -->
          <div v-if="!simplefinConfig.accessUrl" class="space-y-4">
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                SimpleFIN Access URL
              </label>
              <input
                v-model="simplefinAccessUrl"
                type="password"
                placeholder="Paste your access URL from SimpleFIN"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white font-mono text-sm"
              />
              <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                Get this from your SimpleFIN dashboard after connecting your banks.
              </p>
            </div>
            <button
              @click="testSimplefinConnection"
              :disabled="simplefinLoading"
              class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 flex items-center gap-2"
            >
              <svg v-if="simplefinLoading" class="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
              </svg>
              {{ simplefinLoading ? 'Connecting...' : 'Connect SimpleFIN' }}
            </button>
          </div>

          <!-- Connected: show status and sync options -->
          <div v-else class="space-y-4">
            <!-- Connection status -->
            <div class="flex items-center justify-between py-2 px-3 bg-green-50 dark:bg-green-900/20 rounded-lg border border-green-200 dark:border-green-800">
              <div class="flex items-center gap-2">
                <svg class="w-5 h-5 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                <span class="text-sm font-medium text-green-700 dark:text-green-400">SimpleFIN Connected</span>
              </div>
              <span class="text-sm text-gray-500 dark:text-gray-400">
                {{ simplefinConfig.accountMappings.filter(m => m.indibudgetAccountId).length }} of {{ simplefinConfig.accountMappings.length }} accounts mapped
              </span>
            </div>

            <!-- Last sync info -->
            <div class="flex items-center justify-between py-2 px-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
              <span class="text-sm text-gray-600 dark:text-gray-400">Last sync:</span>
              <span class="text-sm font-medium text-gray-900 dark:text-white">{{ formatLastSync(simplefinConfig.lastSync) }}</span>
            </div>

            <!-- Action buttons -->
            <div class="flex flex-wrap gap-3">
              <button
                @click="syncSimplefin"
                :disabled="simplefinLoading"
                class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 flex items-center gap-2"
              >
                <svg v-if="simplefinLoading" class="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
                  <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                  <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
                </svg>
                <svg v-else class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                </svg>
                {{ simplefinLoading ? 'Syncing...' : 'Sync Now' }}
              </button>
              <button
                @click="showAccountMapping = true"
                class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
              >
                Manage Account Mapping
              </button>
              <button
                @click="disconnectSimplefin"
                class="px-4 py-2 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
              >
                Disconnect
              </button>
            </div>

            <!-- Sync results -->
            <div v-if="simplefinSyncResult" class="pt-2">
              <p class="text-sm text-gray-600 dark:text-gray-400">
                Last sync: {{ simplefinSyncResult.imported }} imported, {{ simplefinSyncResult.skipped }} duplicates skipped
              </p>
              <div v-if="simplefinSyncResult.errors.length > 0" class="mt-2 text-sm text-red-600 dark:text-red-400">
                <p class="font-medium">Errors:</p>
                <ul class="list-disc list-inside">
                  <li v-for="(error, i) in simplefinSyncResult.errors.slice(0, 5)" :key="i">{{ error }}</li>
                  <li v-if="simplefinSyncResult.errors.length > 5">
                    ...and {{ simplefinSyncResult.errors.length - 5 }} more
                  </li>
                </ul>
              </div>
            </div>

            <!-- Auto-sync settings -->
            <div class="pt-4 border-t border-gray-200 dark:border-gray-700">
              <h3 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Auto-Sync Settings</h3>
              <div class="space-y-3">
                <div class="flex items-center justify-between">
                  <div>
                    <p class="text-sm text-gray-900 dark:text-white">Enable Auto-Sync</p>
                    <p class="text-xs text-gray-500 dark:text-gray-400">Automatically sync when opening the app</p>
                  </div>
                  <input
                    v-model="simplefinConfig.autoSync"
                    type="checkbox"
                    class="w-5 h-5 rounded border-gray-300 dark:border-gray-600"
                    @change="saveSimplefinConfig"
                  />
                </div>
                <div v-if="simplefinConfig.autoSync" class="flex items-center gap-3">
                  <label class="text-sm text-gray-600 dark:text-gray-400">Sync frequency:</label>
                  <select
                    v-model="simplefinConfig.syncInterval"
                    class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                    @change="saveSimplefinConfig"
                  >
                    <option value="daily">Daily</option>
                    <option value="weekly">Weekly</option>
                  </select>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Account Mapping Modal -->
      <div
        v-if="showAccountMapping"
        class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
        @click.self="showAccountMapping = false"
      >
        <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-2xl w-full mx-4 max-h-[80vh] overflow-hidden">
          <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center">
            <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Map SimpleFIN Accounts</h3>
            <button
              @click="showAccountMapping = false"
              class="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
            >
              <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
          <div class="p-4 overflow-y-auto max-h-[60vh]">
            <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
              Map each SimpleFIN account to an indiBudget account. Transactions will be imported from mapped accounts only.
            </p>

            <div v-if="simplefinConfig.accountMappings.length === 0" class="text-center py-8 text-gray-500">
              No accounts found. Please reconnect SimpleFIN.
            </div>

            <div v-else class="space-y-4">
              <div
                v-for="mapping in simplefinConfig.accountMappings"
                :key="mapping.simplefinAccountId"
                class="p-4 bg-gray-50 dark:bg-gray-700/50 rounded-lg"
              >
                <div class="flex items-start justify-between gap-4">
                  <div class="flex-1">
                    <p class="font-medium text-gray-900 dark:text-white">{{ mapping.simplefinAccountName }}</p>
                    <p class="text-sm text-gray-500 dark:text-gray-400">{{ mapping.simplefinInstitution }}</p>
                  </div>
                  <div class="flex items-center gap-2">
                    <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 8l4 4m0 0l-4 4m4-4H3" />
                    </svg>
                    <select
                      :value="mapping.indibudgetAccountId || ''"
                      @change="updateAccountMapping(mapping.simplefinAccountId, ($event.target as HTMLSelectElement).value || null)"
                      class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm min-w-[200px]"
                    >
                      <option value="">Skip (don't import)</option>
                      <option
                        v-for="account in accountsStore.accounts"
                        :key="account.id"
                        :value="account.id"
                      >
                        {{ account.name }} ({{ account.account_type }})
                      </option>
                    </select>
                  </div>
                </div>
              </div>
            </div>
          </div>
          <div class="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-3">
            <button
              @click="showAccountMapping = false"
              class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
            >
              Close
            </button>
            <button
              @click="showAccountMapping = false; syncSimplefin()"
              class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
            >
              Save & Sync
            </button>
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
