<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useMultiUserStore, type FoundHost } from '../stores/multiuser';
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
// Only used when the host's address on the network has changed since pairing.
const changeAddress = ref(false);
const newAddress = ref('');
const saved = computed(() => store.status.saved_host);

// Computers announcing a budget on this network, so nobody has to type an
// address. Only a convenience: pairing still proves which computer it is.
const found = ref<FoundHost[]>([]);
const looking = ref(false);
const lookedOnce = ref(false);
async function lookForHosts() {
  looking.value = true;
  try {
    found.value = await store.discoverHosts();
  } catch {
    found.value = [];
  } finally {
    looking.value = false;
    lookedOnce.value = true;
  }
}

// While hosting, who is connected changes without any news to say so.
let statusTimer: ReturnType<typeof setInterval> | null = null;
watch(
  () => store.status.hosting,
  (hosting) => {
    if (hosting && !statusTimer) {
      statusTimer = setInterval(() => store.refreshStatus().catch(() => {}), 5000);
    } else if (!hosting && statusTimer) {
      clearInterval(statusTimer);
      statusTimer = null;
    }
  },
  { immediate: true }
);
onBeforeUnmount(() => {
  if (statusTimer) clearInterval(statusTimer);
});

function since(iso: string) {
  const minutes = Math.round((Date.now() - new Date(iso).getTime()) / 60000);
  if (minutes < 1) return 'just now';
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  return `${hours} hour${hours === 1 ? '' : 's'} ago`;
}

function signIn() {
  return run('Could not sign in', () =>
    store
      .connectToHost({
        login: joinLogin.value,
        password: joinPassword.value,
        address: changeAddress.value && newAddress.value ? newAddress.value : null,
      })
      .then(() => {
        joinPassword.value = '';
        changeAddress.value = false;
        return loadShared();
      })
  );
}

function forgetHost() {
  if (!window.confirm('Forget this host? Joining it again will need a new pairing code.')) return;
  return run('Could not forget the host', () => store.forgetSavedHost());
}

const people = ref<any[]>([]);
const devices = ref<any[]>([]);
const closedBy = ref<string | null>(null);

const hostingPort = computed(() => {
  const first = store.status.addresses[0] ?? '';
  return first.split(':').pop() || '7420';
});

const activePeople = computed(() => people.value.filter((p) => p.is_active).length);

const ownCurrent = ref('');
const ownNew = ref('');
async function changeOwnPassword() {
  await run('Could not change your password', async () => {
    await api.changeOwnPassword(ownCurrent.value, ownNew.value);
    ownCurrent.value = '';
    ownNew.value = '';
    notice.value = 'Your password was changed. Use the new one next time you sign in.';
  });
}

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
  if (store.status.saved_host?.last_login) joinLogin.value = store.status.saved_host.last_login;
  if (!store.status.saved_host && !store.isSharing) lookForHosts();
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
      v-if="store.status.lost"
      class="mb-4 px-4 py-3 rounded-lg bg-red-50 dark:bg-red-900/30 text-red-800 dark:text-red-300 text-sm"
    >
      {{ store.status.lost_reason ?? 'The connection to the computer hosting the budget was lost.' }}
      Sign in again below once that is sorted out.
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
          placeholder="7420"
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
        <!-- Already paired with a host: just sign in -->
        <template v-if="saved">
          <h2 class="font-semibold text-gray-900 dark:text-white mb-1">Join the shared budget</h2>
          <p class="text-sm text-gray-600 dark:text-gray-400 mb-1">
            This computer is paired with the budget at
            <code class="px-1 bg-gray-100 dark:bg-gray-800 rounded-sm">{{ saved.address }}</code>.
          </p>
          <p class="text-xs text-gray-500 dark:text-gray-400 mb-4">
            Host identity:
            <code class="break-all">{{ saved.fingerprint_groups }}</code>
          </p>
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
            @keyup.enter="joinLogin && joinPassword && !busy && signIn()"
          />
          <div v-if="changeAddress" class="mb-3">
            <input
              v-model="newAddress"
              placeholder="New address shown on the host, e.g. 192.168.1.20:7420"
              class="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-800"
            />
            <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
              Only the address changes. The host still has to prove it is the same computer you
              paired with.
            </p>
          </div>
          <div class="flex flex-wrap items-center gap-3">
            <button
              :disabled="busy || !joinLogin || !joinPassword"
              class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
              @click="signIn"
            >
              {{ busy ? 'Connecting…' : 'Sign in' }}
            </button>
            <button
              v-if="!changeAddress"
              :disabled="busy"
              class="text-sm text-gray-600 dark:text-gray-400 hover:underline"
              @click="
                changeAddress = true;
                newAddress = saved.address;
              "
            >
              Host's address changed?
            </button>
            <button
              :disabled="busy"
              class="text-sm text-red-600 hover:underline"
              @click="forgetHost"
            >
              Forget this host
            </button>
          </div>
        </template>

        <!-- Not paired yet -->
        <template v-else>
          <h2 class="font-semibold text-gray-900 dark:text-white mb-1">Join a budget</h2>
          <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
            Connect to a budget hosted on another computer. You will need the address and the
            pairing code shown there. You only pair once; after that, signing in is enough.
          </p>
          <div class="mb-3">
            <div class="flex items-center justify-between mb-1">
              <span class="text-sm text-gray-700 dark:text-gray-300">On this network</span>
              <button
                :disabled="looking"
                class="text-sm text-blue-600 dark:text-blue-400 hover:underline disabled:opacity-50"
                @click="lookForHosts"
              >
                {{ looking ? 'Looking…' : 'Look again' }}
              </button>
            </div>
            <ul v-if="found.length" class="space-y-1">
              <li v-for="h in found" :key="h.address">
                <button
                  class="w-full text-left px-3 py-2 rounded-lg border text-sm"
                  :class="
                    joinAddress === h.address
                      ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/30'
                      : 'border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800'
                  "
                  @click="joinAddress = h.address"
                >
                  <span class="font-medium text-gray-900 dark:text-white">indiBudget on {{ h.computer }}</span>
                  <span class="block text-xs text-gray-500 dark:text-gray-400">{{ h.address }}</span>
                </button>
              </li>
            </ul>
            <p v-else-if="lookedOnce && !looking" class="text-xs text-gray-500 dark:text-gray-400">
              No computer this one can hear is hosting a budget. Check the host has started
              hosting, or type its address below.
            </p>
          </div>
          <input
            v-model="joinAddress"
            placeholder="Address shown on the host, e.g. 192.168.1.20:7420"
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
            :disabled="busy || !joinAddress || !joinCode"
            class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
            @click="run('Could not pair', () => store.pairWithHost(joinAddress, joinCode, joinLabel))"
          >
            {{ busy ? 'Pairing…' : 'Pair' }}
          </button>
        </template>
      </div>
    </section>

    <!-- Hosting -->
    <section v-else-if="mode === 'hosting'" class="space-y-4">
      <div class="p-5 rounded-xl border border-green-200 dark:border-green-800 bg-green-50 dark:bg-green-900/20">
        <div class="flex items-start justify-between gap-4">
          <div>
            <h2 class="font-semibold text-gray-900 dark:text-white">Hosting</h2>
            <div v-if="store.status.addresses.length" class="text-sm text-gray-700 dark:text-gray-300 mt-1">
              Others can connect to
              <code class="px-1 bg-white dark:bg-gray-800 rounded-sm">{{ store.status.addresses[0] }}</code>
              <p v-if="store.status.addresses.length > 1" class="text-xs text-gray-600 dark:text-gray-400 mt-1">
                This computer has more than one network connection. If that address
                does not work, try
                <template v-for="(a, i) in store.status.addresses.slice(1)" :key="a">
                  <span v-if="i > 0">, </span><code class="px-1 bg-white dark:bg-gray-800 rounded-sm">{{ a }}</code>
                </template>.
              </p>
            </div>
            <p v-else class="text-sm text-amber-700 dark:text-amber-300 mt-1">
              This computer does not seem to be connected to a network, so others
              cannot reach it yet.
            </p>
            <p class="text-xs text-gray-600 dark:text-gray-400 mt-2">
              If a firewall is running here, allow incoming TCP connections on port
              {{ hostingPort }}.
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

      <div class="p-5 rounded-xl border border-gray-200 dark:border-gray-700">
        <h2 class="font-semibold text-gray-900 dark:text-white mb-3">Connected now</h2>
        <ul v-if="store.status.connected_people.length" class="divide-y divide-gray-100 dark:divide-gray-800">
          <li
            v-for="(seat, i) in store.status.connected_people"
            :key="i"
            class="py-2 flex items-center justify-between text-sm"
          >
            <span>
              <span class="font-medium text-gray-900 dark:text-white">{{ seat.person }}</span>
              <span class="text-gray-600 dark:text-gray-400"> on {{ seat.computer }}</span>
            </span>
            <span class="text-xs text-gray-500 dark:text-gray-400">active {{ since(seat.last_active_at) }}</span>
          </li>
        </ul>
        <p v-else class="text-sm text-gray-600 dark:text-gray-400">
          Nobody is signed in from another computer right now.
        </p>
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
          Revoking takes effect at once: that computer is disconnected on its next click.
        </p>
      </div>

      <div
        class="p-5 rounded-xl border"
        :class="
          activePeople
            ? 'border-gray-200 dark:border-gray-700'
            : 'border-amber-300 dark:border-amber-700 bg-amber-50 dark:bg-amber-900/20'
        "
      >
        <h2 class="font-semibold text-gray-900 dark:text-white mb-1">People</h2>
        <p v-if="activePeople" class="text-sm text-gray-700 dark:text-gray-300">
          {{ activePeople }} {{ activePeople === 1 ? 'person' : 'people' }} can sign in from another
          computer.
        </p>
        <p v-else class="text-sm text-amber-800 dark:text-amber-300">
          Nobody can sign in from another computer yet. Pairing a computer only lets it ask; each
          person also needs a login of their own.
        </p>
        <router-link
          to="/people"
          class="inline-block mt-3 px-4 py-2 rounded-lg text-sm bg-blue-600 text-white hover:bg-blue-700"
        >
          {{ activePeople ? 'Manage people' : 'Add people' }}
        </router-link>
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

      <details class="mt-4 pt-4 border-t border-blue-200 dark:border-blue-800">
        <summary class="text-sm text-gray-800 dark:text-gray-200 cursor-pointer">Change your password</summary>
        <div class="mt-3 grid gap-2 max-w-sm">
          <input
            v-model="ownCurrent"
            type="password"
            placeholder="Current password"
            class="px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-800"
          />
          <input
            v-model="ownNew"
            type="password"
            placeholder="New password"
            class="px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-800"
          />
          <p class="text-xs text-gray-600 dark:text-gray-400">
            At least 8 characters, with an uppercase letter, a lowercase letter and a number.
          </p>
          <button
            :disabled="busy || !ownCurrent || !ownNew"
            class="justify-self-start px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
            @click="changeOwnPassword"
          >
            Change password
          </button>
        </div>
      </details>
    </section>
  </div>
</template>
