<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useMultiUserStore } from '../stores/multiuser';
import * as api from '../services/api';

const store = useMultiUserStore();

const port = ref<number | null>(null);
const busy = ref(false);
const notice = ref<string | null>(null);

// Connecting to someone else's budget.
const joinAddress = ref('');
const joinCode = ref('');
const joinLabel = ref('This computer');
const joinLogin = ref('');
const joinPassword = ref('');
const paired = ref<{ device_token: string; fingerprint: string; fingerprint_groups: string } | null>(
  null
);

const people = ref<any[]>([]);
const devices = ref<any[]>([]);
const closedBy = ref<string | null>(null);

const mode = computed(() => {
  if (store.status.connected) return 'connected';
  if (store.status.hosting) return 'hosting';
  return 'idle';
});

async function run(label: string, fn: () => Promise<unknown>) {
  busy.value = true;
  notice.value = null;
  try {
    await fn();
  } catch (e) {
    notice.value = `${label}: ${e instanceof Error ? e.message : String(e)}`;
  } finally {
    busy.value = false;
  }
}

async function loadShared() {
  if (!store.isSharing) return;
  try {
    people.value = await api.listUsers();
    devices.value = await api.listDevices();
    const status = await api.maintenanceStatus();
    closedBy.value = status?.closed_by ?? null;
  } catch {
    // A member without the Admin grant cannot list people; that is expected
    // and is not worth an error banner.
    people.value = [];
    devices.value = [];
  }
}

onMounted(async () => {
  await store.refreshStatus();
  if (store.isSharing) {
    store.startBeat();
    await loadShared();
  }
});
</script>

<template>
  <div class="p-8 max-w-4xl">
    <header class="mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Sharing</h1>
      <p class="text-sm text-gray-600 dark:text-gray-400 mt-1">
        Let another computer on your home network work from this same budget.
      </p>
    </header>

    <div
      v-if="notice"
      class="mb-4 px-4 py-3 rounded-lg bg-amber-50 dark:bg-amber-900/30 text-amber-800 dark:text-amber-300 text-sm"
    >
      {{ notice }}
    </div>

    <div
      v-if="store.isClosed"
      class="mb-4 px-4 py-3 rounded-lg bg-blue-50 dark:bg-blue-900/30 text-blue-800 dark:text-blue-300 text-sm"
    >
      {{ store.maintenanceClosedBy }} has closed this budget for maintenance. You can still look at
      it, but changes are paused.
    </div>

    <!-- Nothing shared yet -->
    <section v-if="mode === 'idle'" class="grid gap-4 md:grid-cols-2">
      <div class="p-5 rounded-xl border border-gray-200 dark:border-gray-700">
        <h2 class="font-semibold text-gray-900 dark:text-white mb-1">Host this budget</h2>
        <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
          This computer keeps the data and others connect to it. It needs to stay awake and on the
          same network.
        </p>
        <label class="block text-sm text-gray-700 dark:text-gray-300 mb-1">Port (optional)</label>
        <input
          v-model.number="port"
          type="number"
          placeholder="Choose automatically"
          class="w-full mb-3 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-800"
        />
        <button
          :disabled="busy"
          class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
          @click="run('Could not start hosting', () => store.startHosting(port ?? undefined).then(loadShared))"
        >
          Start hosting
        </button>
      </div>

      <div class="p-5 rounded-xl border border-gray-200 dark:border-gray-700">
        <h2 class="font-semibold text-gray-900 dark:text-white mb-1">Join a budget</h2>
        <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
          Connect to a budget hosted on another computer. You will need the address and the pairing
          code shown there.
        </p>
        <input
          v-model="joinAddress"
          placeholder="192.168.1.20:7420"
          class="w-full mb-2 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-800"
        />
        <input
          v-model="joinCode"
          placeholder="Pairing code"
          class="w-full mb-2 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-800"
        />
        <input
          v-model="joinLabel"
          placeholder="Name for this computer"
          class="w-full mb-3 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-800"
        />
        <button
          v-if="!paired"
          :disabled="busy || !joinAddress || !joinCode"
          class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
          @click="
            run('Could not pair', async () => {
              paired = await store.pairWithHost(joinAddress, joinCode, joinLabel);
            })
          "
        >
          Pair
        </button>

        <div v-else>
          <p class="text-sm text-gray-600 dark:text-gray-400 mb-2">
            Paired. Check this matches the code shown on the other computer:
          </p>
          <code
            class="block mb-3 p-2 text-xs bg-gray-100 dark:bg-gray-800 rounded break-all"
            >{{ paired.fingerprint_groups }}</code
          >
          <input
            v-model="joinLogin"
            placeholder="Your login"
            class="w-full mb-2 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-800"
          />
          <input
            v-model="joinPassword"
            type="password"
            placeholder="Your password"
            class="w-full mb-3 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-800"
          />
          <button
            :disabled="busy || !joinLogin || !joinPassword"
            class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
            @click="
              run('Could not sign in', () =>
                store
                  .connectToHost({
                    address: joinAddress,
                    fingerprint: paired!.fingerprint,
                    device_token: paired!.device_token,
                    login: joinLogin,
                    password: joinPassword,
                  })
                  .then(loadShared)
              )
            "
          >
            Sign in
          </button>
        </div>
      </div>
    </section>

    <!-- Hosting -->
    <section v-else-if="mode === 'hosting'" class="space-y-4">
      <div class="p-5 rounded-xl border border-green-200 dark:border-green-800 bg-green-50 dark:bg-green-900/20">
        <div class="flex items-start justify-between gap-4">
          <div>
            <h2 class="font-semibold text-gray-900 dark:text-white">Hosting</h2>
            <p class="text-sm text-gray-700 dark:text-gray-300 mt-1">
              Others can connect to
              <code class="px-1 bg-white dark:bg-gray-800 rounded">{{ store.status.address }}</code>
            </p>
            <p class="text-xs text-gray-600 dark:text-gray-400 mt-2">
              Identity code — read this out to confirm they reached the right computer:
            </p>
            <code class="block mt-1 text-xs break-all">{{ store.status.fingerprint_groups }}</code>
          </div>
          <button
            :disabled="busy"
            class="px-3 py-2 rounded-lg text-sm text-gray-700 dark:text-gray-300 hover:bg-white dark:hover:bg-gray-800"
            @click="run('Could not stop hosting', () => store.stopHosting())"
          >
            Stop hosting
          </button>
        </div>
      </div>

      <div class="p-5 rounded-xl border border-gray-200 dark:border-gray-700">
        <h2 class="font-semibold text-gray-900 dark:text-white mb-2">Add a computer</h2>
        <div v-if="store.pairingCode" class="mb-3">
          <p class="text-sm text-gray-600 dark:text-gray-400 mb-1">
            Type this on the other computer. It expires in a few minutes.
          </p>
          <code class="text-2xl font-bold tracking-widest">{{ store.pairingCode }}</code>
        </div>
        <div class="flex gap-2">
          <button
            :disabled="busy"
            class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
            @click="run('Could not start pairing', () => store.openPairing())"
          >
            {{ store.pairingCode ? 'New code' : 'Start pairing' }}
          </button>
          <button
            v-if="store.pairingCode"
            :disabled="busy"
            class="px-4 py-2 rounded-lg text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800"
            @click="run('Could not stop pairing', () => store.closePairing())"
          >
            Stop pairing
          </button>
        </div>
      </div>

      <div class="p-5 rounded-xl border border-gray-200 dark:border-gray-700">
        <h2 class="font-semibold text-gray-900 dark:text-white mb-3">Maintenance</h2>
        <p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
          Closing pauses everyone's changes while you take a backup. People can still look at the
          budget, and any administrator can reopen it.
        </p>
        <button
          v-if="!closedBy"
          :disabled="busy"
          class="px-4 py-2 rounded-lg border border-gray-300 dark:border-gray-600"
          @click="run('Could not close', () => api.maintenanceClose().then(loadShared))"
        >
          Close for maintenance
        </button>
        <button
          v-else
          :disabled="busy"
          class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700"
          @click="run('Could not reopen', () => api.maintenanceReopen().then(loadShared))"
        >
          Reopen ({{ closedBy }} closed it)
        </button>
      </div>

      <div v-if="devices.length" class="p-5 rounded-xl border border-gray-200 dark:border-gray-700">
        <h2 class="font-semibold text-gray-900 dark:text-white mb-3">Paired computers</h2>
        <ul class="divide-y divide-gray-100 dark:divide-gray-800">
          <li v-for="d in devices" :key="d.id" class="py-2 flex items-center justify-between">
            <span :class="d.is_revoked ? 'line-through text-gray-400' : ''">{{ d.label }}</span>
            <button
              v-if="!d.is_revoked"
              class="text-sm text-red-600 hover:underline"
              @click="run('Could not revoke', () => api.revokeDevice(d.id).then(loadShared))"
            >
              Revoke
            </button>
          </li>
        </ul>
        <p class="text-xs text-gray-500 mt-2">
          Revoking takes effect the next time that computer connects.
        </p>
      </div>

      <div v-if="people.length" class="p-5 rounded-xl border border-gray-200 dark:border-gray-700">
        <h2 class="font-semibold text-gray-900 dark:text-white mb-3">People</h2>
        <ul class="divide-y divide-gray-100 dark:divide-gray-800">
          <li v-for="p in people" :key="p.id" class="py-2 flex items-center justify-between">
            <span>
              {{ p.display_name }}
              <span class="text-xs text-gray-500">({{ p.login }})</span>
              <span v-if="p.is_owner" class="ml-2 text-xs text-blue-600">administrator</span>
            </span>
            <span v-if="!p.is_active" class="text-xs text-gray-400">deactivated</span>
          </li>
        </ul>
      </div>
    </section>

    <!-- Connected to someone else -->
    <section v-else class="p-5 rounded-xl border border-blue-200 dark:border-blue-800 bg-blue-50 dark:bg-blue-900/20">
      <div class="flex items-start justify-between gap-4">
        <div>
          <h2 class="font-semibold text-gray-900 dark:text-white">
            Connected as {{ store.status.signed_in_as }}
          </h2>
          <p class="text-sm text-gray-700 dark:text-gray-300 mt-1">
            You are working from a budget hosted on another computer. Backups, imports and
            encryption stay on that machine.
          </p>
        </div>
        <button
          :disabled="busy"
          class="px-3 py-2 rounded-lg text-sm text-gray-700 dark:text-gray-300 hover:bg-white dark:hover:bg-gray-800"
          @click="run('Could not disconnect', () => store.disconnect())"
        >
          Disconnect
        </button>
      </div>
    </section>
  </div>
</template>
