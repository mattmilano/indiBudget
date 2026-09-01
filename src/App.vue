<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useRoute } from 'vue-router';
import UserAgreementModal from './components/UserAgreementModal.vue';
import { navItems } from './navigation';
import * as api from './services/api';

const route = useRoute();

const sidebarOpen = ref(true);
const showAgreement = ref(false);
const appReady = ref(false);

const CURRENT_AGREEMENT_VERSION = '1.0';

onMounted(async () => {
  try {
    const accepted = await api.getSetting('user_agreement_accepted');
    const version = await api.getSetting('user_agreement_version');

    if (accepted !== 'true' || version !== CURRENT_AGREEMENT_VERSION) {
      showAgreement.value = true;
    } else {
      appReady.value = true;
    }
  } catch {
    showAgreement.value = true;
  }
});

function onAgreementAccepted() {
  showAgreement.value = false;
  appReady.value = true;
}


const isActive = (path: string) => route.path === path;
</script>

<template>
  <!-- User Agreement Modal -->
  <UserAgreementModal :show="showAgreement" @accepted="onAgreementAccepted" />

  <div v-if="appReady" class="flex h-screen bg-gray-100 dark:bg-gray-900">
    <!-- Sidebar - Always dark like indiAccounting -->
    <aside
      :class="[
        'fixed inset-y-0 left-0 z-50 flex flex-col bg-gray-900 shadow-lg transition-all duration-300',
        sidebarOpen ? 'w-64' : 'w-20'
      ]"
    >
      <!-- Logo -->
      <div class="flex items-center justify-between h-16 px-4 border-b border-gray-700">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 bg-blue-600 rounded-lg flex items-center justify-center">
            <svg class="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          </div>
          <span v-if="sidebarOpen" class="text-xl font-bold text-white">indiBudget</span>
        </div>
        <button
          @click="sidebarOpen = !sidebarOpen"
          class="p-2 text-gray-400 hover:text-gray-200"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              :d="sidebarOpen ? 'M11 19l-7-7 7-7m8 14l-7-7 7-7' : 'M13 5l7 7-7 7M5 5l7 7-7 7'"
            />
          </svg>
        </button>
      </div>

      <!-- Navigation -->
      <nav class="flex-1 overflow-y-auto py-4">
        <ul class="space-y-1 px-3">
          <li v-for="item in navItems" :key="item.path">
            <router-link
              :to="item.path"
              :class="[
                'flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors',
                isActive(item.path)
                  ? 'bg-blue-600 text-white'
                  : 'text-gray-300 hover:bg-gray-800 hover:text-white'
              ]"
            >
              <svg class="w-5 h-5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" :d="item.icon" />
              </svg>
              <span v-if="sidebarOpen" class="font-medium">{{ item.name }}</span>
            </router-link>
          </li>
        </ul>
      </nav>

      <!-- Footer -->
      <div v-if="sidebarOpen" class="p-4 border-t border-gray-700">
        <p class="text-xs text-gray-500 text-center">
          indiBudget v1.0.0
        </p>
      </div>
    </aside>

    <!-- Main Content -->
    <main
      :class="[
        'flex-1 overflow-y-auto transition-all duration-300',
        sidebarOpen ? 'ml-64' : 'ml-20'
      ]"
    >
      <router-view />
    </main>
  </div>
</template>

<style>
@media (prefers-color-scheme: dark) {
  :root {
    color-scheme: dark;
  }
}
</style>
