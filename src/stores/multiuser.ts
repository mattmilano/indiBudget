/**
 * Hosting, connecting, and the five-second beat.
 *
 * The beat asks "what changed since my mark?" and gets back things to re-read,
 * never rows. Screens listen for the kinds they care about and re-fetch through
 * the read paths they always use, so every grant is checked again on the way.
 */
import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { invokeLocal, setConnectedToHost } from '../lib/rpc';
import * as api from '../services/api';

/** Matches `NEWS_BEAT_SECONDS` in the Rust boundary. */
export const NEWS_BEAT_MS = 5000;

export interface HostingStatus {
  hosting: boolean;
  address: string | null;
  /** Where other computers can reach this one, best first. */
  addresses: string[];
  fingerprint: string | null;
  fingerprint_groups: string | null;
  pairing: boolean;
  connected: boolean;
  signed_in_as: string | null;
  /** The host this computer paired with, remembered across restarts. */
  saved_host: SavedHost | null;
  /** The connection to the host dropped and has not been re-established. */
  lost: boolean;
  /** Why, in a sentence: the host went away, or ended this person's session. */
  lost_reason: string | null;
  /** While hosting: who is signed in from other computers right now. */
  connected_people: Seat[];
}

/** Someone signed in from another computer. */
export interface Seat {
  person: string;
  computer: string;
  connected_at: string;
  last_active_at: string;
}

/** A computer heard announcing a budget on this network. */
export interface FoundHost {
  computer: string;
  address: string;
  /** A hint for the screen only; pairing proves which computer it is. */
  fingerprint_groups: string;
}

export interface SavedHost {
  address: string;
  fingerprint_groups: string;
  label: string;
  last_login: string | null;
}

export interface Notice {
  kind:
    | 'record_changed'
    | 'record_busy'
    | 'record_freed'
    | 'people_changed'
    | 'maintenance_on'
    | 'maintenance_off';
  area?: string;
  record_kind?: string;
  record_id?: string;
  holder?: string;
  closed_by?: string;
}

interface Mark {
  run: string;
  seq: number;
}

type CatchUp =
  | { status: 'notices'; notices: Notice[]; mark: Mark }
  | { status: 'start_over'; mark: Mark };

export const useMultiUserStore = defineStore('multiuser', () => {
  const status = ref<HostingStatus>({
    hosting: false,
    address: null,
    addresses: [],
    fingerprint: null,
    fingerprint_groups: null,
    pairing: false,
    connected: false,
    signed_in_as: null,
    saved_host: null,
    lost: false,
    lost_reason: null,
    connected_people: [],
  });

  const pairingCode = ref<string | null>(null);
  const maintenanceClosedBy = ref<string | null>(null);
  const error = ref<string | null>(null);

  /** Record id → who is editing it, for the badges. */
  const busy = ref<Record<string, string>>({});

  /** Bumped whenever something changed, so screens can watch one number. */
  const changeTick = ref(0);

  /** Bumped when anyone's access changed, for the People screen. */
  const peopleTick = ref(0);

  /** Set when the host restarted or we fell too far behind. */
  const needsFullRefresh = ref(false);

  let mark: Mark | null = null;
  let beat: ReturnType<typeof setInterval> | null = null;

  const isSharing = computed(() => status.value.hosting || status.value.connected);
  const isClosed = computed(() => maintenanceClosedBy.value !== null);

  function busyKey(recordKind: string, recordId: string) {
    return `${recordKind}:${recordId}`;
  }

  function holderOf(recordKind: string, recordId: string): string | null {
    return busy.value[busyKey(recordKind, recordId)] ?? null;
  }

  async function refreshStatus() {
    status.value = await invokeLocal<HostingStatus>('hosting_status');
    setConnectedToHost(status.value.connected);
  }

  /** Listen for a couple of seconds for computers hosting a budget nearby. */
  async function discoverHosts() {
    return invokeLocal<FoundHost[]>('discover_hosts');
  }

  async function startHosting(port?: number) {
    error.value = null;
    try {
      status.value = await invokeLocal<HostingStatus>('start_hosting', { port: port ?? null });
      startBeat();
    } catch (e) {
      error.value = String(e);
      throw e;
    }
  }

  async function stopHosting() {
    status.value = await invokeLocal<HostingStatus>('stop_hosting');
    pairingCode.value = null;
    stopBeat();
  }

  async function openPairing() {
    error.value = null;
    try {
      pairingCode.value = await invokeLocal<string>('open_pairing');
      await refreshStatus();
    } catch (e) {
      error.value = String(e);
      throw e;
    }
  }

  async function closePairing() {
    await invokeLocal('close_pairing');
    pairingCode.value = null;
    await refreshStatus();
  }

  async function pairWithHost(address: string, code: string, label: string) {
    error.value = null;
    try {
      // The host is remembered on the Rust side, token included; nothing
      // secret comes back to the page.
      status.value = await invokeLocal<HostingStatus>('pair_with_host', {
        request: { address, code, label },
      });
    } catch (e) {
      error.value = String(e);
      throw e;
    }
  }

  async function connectToHost(request: {
    login: string;
    password: string;
    /** Only when the host's address changed since pairing. */
    address?: string | null;
  }) {
    error.value = null;
    try {
      status.value = await invokeLocal<HostingStatus>('connect_to_host', { request });
      setConnectedToHost(true);
      needsFullRefresh.value = true;
      startBeat();
    } catch (e) {
      error.value = String(e);
      throw e;
    }
  }

  async function forgetSavedHost() {
    status.value = await invokeLocal<HostingStatus>('forget_saved_host');
    setConnectedToHost(false);
    stopBeat();
  }

  async function disconnect() {
    status.value = await invokeLocal<HostingStatus>('disconnect_from_host');
    setConnectedToHost(false);
    stopBeat();
  }

  function apply(notice: Notice) {
    switch (notice.kind) {
      case 'record_changed':
        changeTick.value += 1;
        break;
      case 'record_busy':
        if (notice.record_kind && notice.record_id && notice.holder) {
          busy.value[busyKey(notice.record_kind, notice.record_id)] = notice.holder;
        }
        break;
      case 'record_freed':
        if (notice.record_kind && notice.record_id) {
          delete busy.value[busyKey(notice.record_kind, notice.record_id)];
        }
        break;
      case 'maintenance_on':
        maintenanceClosedBy.value = notice.closed_by ?? 'An administrator';
        break;
      case 'maintenance_off':
        maintenanceClosedBy.value = null;
        break;
      case 'people_changed':
        // Someone's access moved. What this window may see could have
        // shifted, so everything on screen is re-read.
        peopleTick.value += 1;
        changeTick.value += 1;
        break;
    }
  }

  async function catchUp() {
    try {
      const result = await api.newsCatchUp(mark);
      mark = result.mark;

      if (result.status === 'start_over') {
        // The host restarted, or we were away too long. Both arrive here, and
        // both are answered the same way: forget the badges and re-read.
        busy.value = {};
        needsFullRefresh.value = true;
        changeTick.value += 1;
        return;
      }
      result.notices.forEach(apply);
    } catch {
      // One missed beat is not worth a message, but it may mean the host has
      // gone. Re-read the status: if the connection was lost, the Sharing
      // screen and the banner say so, and the beat stops asking.
      await refreshStatus().catch(() => {});
      if (status.value.lost) stopBeat();
    }
  }

  function startBeat() {
    if (beat) return;
    void catchUp();
    beat = setInterval(() => void catchUp(), NEWS_BEAT_MS);
  }

  function stopBeat() {
    if (beat) clearInterval(beat);
    beat = null;
    busy.value = {};
    maintenanceClosedBy.value = null;
    mark = null;
  }

  function acknowledgeRefresh() {
    needsFullRefresh.value = false;
  }

  return {
    discoverHosts,
    status,
    pairingCode,
    maintenanceClosedBy,
    error,
    busy,
    changeTick,
    peopleTick,
    needsFullRefresh,
    isSharing,
    isClosed,
    holderOf,
    refreshStatus,
    startHosting,
    stopHosting,
    openPairing,
    closePairing,
    pairWithHost,
    connectToHost,
    disconnect,
    forgetSavedHost,
    catchUp,
    startBeat,
    stopBeat,
    acknowledgeRefresh,
  };
});

export type { CatchUp, Mark };
