/**
 * An edit hold for a screen.
 *
 * Self-acquiring on mount and released on unmount, so wiring an edit screen is
 * one line rather than a ceremony:
 *
 * ```ts
 * const lease = useLease('budget', () => budgetId.value);
 * ```
 *
 * Renews on a heartbeat comfortably inside the server's timeout, so one dropped
 * beat does not drop the hold. Transactions deliberately take no hold — see the
 * Rust `boundary::leases` module for why.
 */
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { BoundaryError } from '../lib/rpc';
import * as api from '../services/api';

export type LeasableKind = 'account' | 'budget' | 'category' | 'goal';

/** Matches `LEASE_HEARTBEAT` in the Rust boundary. */
const HEARTBEAT_MS = 20_000;

export function useLease(kind: LeasableKind, recordId: () => string | null | undefined) {
  const held = ref(false);
  /** Who has it, when we could not get it. */
  const heldBy = ref<string | null>(null);
  const message = ref<string | null>(null);

  let timer: ReturnType<typeof setInterval> | null = null;
  let current: string | null = null;

  async function acquire() {
    const id = recordId();
    if (!id) return;
    current = id;
    try {
      await api.leaseAcquire(kind, id);
      held.value = true;
      heldBy.value = null;
      message.value = null;
      startHeartbeat();
    } catch (e) {
      held.value = false;
      if (e instanceof BoundaryError) {
        message.value = e.message;
        heldBy.value = e.isBusy ? ((e.detail.holder as string) ?? null) : null;
      } else {
        message.value = String(e);
      }
    }
  }

  async function release() {
    stopHeartbeat();
    if (!held.value || !current) return;
    held.value = false;
    try {
      await api.leaseRelease(kind, current);
    } catch {
      // Letting go is best-effort: the hold expires on its own, and a dropped
      // connection releases it at the host.
    }
  }

  function startHeartbeat() {
    if (timer) return;
    timer = setInterval(() => {
      if (!current) return;
      api.leaseRenew(kind, current).catch((e) => {
        // Someone took it while we were away. Say so rather than letting the
        // person discover it when their save is refused.
        held.value = false;
        stopHeartbeat();
        if (e instanceof BoundaryError) {
          message.value = e.message;
          heldBy.value = (e.detail.holder as string) ?? null;
        }
      });
    }, HEARTBEAT_MS);
  }

  function stopHeartbeat() {
    if (timer) clearInterval(timer);
    timer = null;
  }

  onMounted(acquire);
  onBeforeUnmount(release);

  // Following the id means a screen that switches records hands the old one
  // back instead of sitting on it.
  watch(recordId, async (next, previous) => {
    if (next === previous) return;
    if (previous) {
      const was = current;
      current = previous;
      await release();
      current = was;
    }
    await acquire();
  });

  return { held, heldBy, message, acquire, release };
}
