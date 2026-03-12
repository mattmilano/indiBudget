<script setup lang="ts">
import { ref, onMounted } from 'vue';
import * as api from '../services/api';

const databasePath = ref('');
const transactionCount = ref(0);

onMounted(async () => {
  try {
    databasePath.value = await api.getDatabasePath();
    transactionCount.value = await api.getTransactionCount();
  } catch (e) {
    console.error('Failed to get debug info:', e);
  }
});
</script>

<template>
  <div class="p-6">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white mb-6">About</h1>

    <div class="max-w-3xl">
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <!-- App Header -->
        <div class="flex items-center gap-4 mb-6">
          <div class="w-20 h-20 bg-gradient-to-br from-blue-500 to-indigo-600 rounded-2xl flex items-center justify-center shadow-lg">
            <svg class="w-12 h-12 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          </div>
          <div>
            <h2 class="text-3xl font-bold text-gray-900 dark:text-white">indiBudget</h2>
            <p class="text-lg text-gray-500 dark:text-gray-400">Version 1.0.0</p>
          </div>
        </div>

        <p class="text-gray-600 dark:text-gray-300 text-lg mb-8">
          A powerful personal finance application with calendar-focused expense tracking,
          designed to help you take control of your money with intuitive tools and insights.
        </p>

        <!-- Features -->
        <div class="mb-8">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white uppercase tracking-wide mb-4">Features</h3>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div class="flex items-center gap-3 text-gray-600 dark:text-gray-300">
              <svg class="w-5 h-5 text-green-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
              </svg>
              Calendar-based transaction view
            </div>
            <div class="flex items-center gap-3 text-gray-600 dark:text-gray-300">
              <svg class="w-5 h-5 text-green-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
              </svg>
              Multiple account management
            </div>
            <div class="flex items-center gap-3 text-gray-600 dark:text-gray-300">
              <svg class="w-5 h-5 text-green-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
              </svg>
              Budget tracking & alerts
            </div>
            <div class="flex items-center gap-3 text-gray-600 dark:text-gray-300">
              <svg class="w-5 h-5 text-green-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
              </svg>
              Recurring transaction detection
            </div>
            <div class="flex items-center gap-3 text-gray-600 dark:text-gray-300">
              <svg class="w-5 h-5 text-green-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
              </svg>
              Savings goals & progress tracking
            </div>
            <div class="flex items-center gap-3 text-gray-600 dark:text-gray-300">
              <svg class="w-5 h-5 text-green-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
              </svg>
              Smart auto-categorization
            </div>
            <div class="flex items-center gap-3 text-gray-600 dark:text-gray-300">
              <svg class="w-5 h-5 text-green-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
              </svg>
              CSV/Excel import & export
            </div>
            <div class="flex items-center gap-3 text-gray-600 dark:text-gray-300">
              <svg class="w-5 h-5 text-green-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
              </svg>
              PDF & CSV report export
            </div>
            <div class="flex items-center gap-3 text-gray-600 dark:text-gray-300">
              <svg class="w-5 h-5 text-green-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
              </svg>
              Net worth tracking & history
            </div>
            <div class="flex items-center gap-3 text-gray-600 dark:text-gray-300">
              <svg class="w-5 h-5 text-green-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
              </svg>
              SimpleFIN bank sync
            </div>
            <div class="flex items-center gap-3 text-gray-600 dark:text-gray-300">
              <svg class="w-5 h-5 text-green-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
              </svg>
              Database encryption
            </div>
            <div class="flex items-center gap-3 text-gray-600 dark:text-gray-300">
              <svg class="w-5 h-5 text-green-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
              </svg>
              Encrypted backups
            </div>
          </div>
        </div>

        <!-- Credits -->
        <div class="pt-6 border-t border-gray-200 dark:border-gray-700">
          <div class="flex flex-wrap gap-6 text-sm mb-4">
            <div>
              <span class="text-gray-500 dark:text-gray-400">Author:</span>
              <span class="ml-2 text-gray-700 dark:text-gray-300 font-medium">Matt Milano</span>
            </div>
            <div>
              <span class="text-gray-500 dark:text-gray-400">License:</span>
              <span class="ml-2 text-gray-700 dark:text-gray-300 font-medium">MIT</span>
            </div>
          </div>
          <div class="flex gap-4">
            <a
              href="https://github.com/mattmilano/indiBudget"
              target="_blank"
              class="inline-flex items-center gap-2 px-4 py-2 bg-gray-900 dark:bg-gray-700 text-white rounded-lg hover:bg-gray-800 dark:hover:bg-gray-600 transition-colors"
            >
              <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
                <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
              </svg>
              View on GitHub
            </a>
          </div>
        </div>

        <!-- Debug Info -->
        <div class="mt-6 pt-6 border-t border-gray-200 dark:border-gray-700">
          <h3 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-3">Debug Information</h3>
          <div class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4 space-y-2">
            <p class="text-sm text-gray-600 dark:text-gray-400">
              <span class="font-medium">Database:</span>
              <span class="ml-2 font-mono text-xs break-all">{{ databasePath }}</span>
            </p>
            <p class="text-sm text-gray-600 dark:text-gray-400">
              <span class="font-medium">Transactions:</span>
              <span class="ml-2">{{ transactionCount.toLocaleString() }}</span>
            </p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
