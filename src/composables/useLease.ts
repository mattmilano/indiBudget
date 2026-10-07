/**
 * An edit hold for a screen.
 *
 * Follows the record being edited, so wiring an edit dialog is one line rather
 * than a ceremony:
 *
 * ```ts
 * const lease = useLease('budget', () => (showEdit.value ? budget.value?.id : null));
 * ```
 *
 * The getter returns the id of the record on screen, or nothing when no edit is
 * open. The hold is taken when an id appears, handed back when it changes or
 * goes away, and handed back when the screen closes.
 *
 * Renews on a heartbeat comfortably inside the server's timeout, so one dropped
 * beat does not drop the hold. While someone else has it, the same beat asks
 * again, so the form opens up by itself once they have finished. Transactions
 * deliberately take no hold — see the Rust `boundary::leases` module for why.
 */
import { onScopeDispose, ref, watch } from 'vue';
import { BoundaryError } from '../lib/rpc';
import * as api from '../services/api';

export type LeasableKind = 'account' | 'budget' | 'category' | 'goal';

/** Matches `LEASE_HEARTBEAT` in the Rust boundary. */
export const HEARTBEAT_MS = 20_000;

/** When a hold was refused: as the form opened, or later on a renewal. */
export type RefusedAt = 'acquire' | 'renew';

export interface LeaseOptions {
  /**
   * Called when the hold is finally had after being refused for the same
   * record. `refusedAt` tells the screen what the person has seen: refused at
   * `'acquire'` means the form was locked from the start and nothing was typed,
   * so the screen should load the latest saved version before unlocking;
   * refused at `'renew'` means the person was part-way through, and what they
   * typed must be left alone.
   */
  onRegained?: (recordId: string, refusedAt: RefusedAt) => void;
}

export function useLease(
  kind: LeasableKind,
  recordId: () => string | null | undefined,
  options: LeaseOptions = {}
) {
  /** True only while the host has confirmed this screen holds the record. */
  const held = ref(false);
  /** True while asking for the hold, before any answer. */
  const pending = ref(false);
  /** Who has it, when we could not get it. */
  const heldBy = ref<string | null>(null);
  /** The sentence to show when we could not get it, or lost it. */
  const message = ref<string | null>(null);

  /** The record the host has actually given us, whatever the screen now shows. */
  let holding: string | null = null;
  /** The record we were last refused, and when. Cleared once it is had. */
  let refused: { id: string; at: RefusedAt } | null = null;
  let disposed = false;
  let timer: ReturnType<typeof setInterval> | null = null;

  /**
   * Every exchange with the host runs one after another.
   *
   * Without this, closing a dialog while its hold was still being granted let
   * the grant land after the release: the screen then kept renewing a hold on
   * a record nobody had open, blocking everyone else indefinitely. Likewise
   * reopening the same record could see its release overtake the new acquire.
   * Queuing means each step starts from what the previous one actually left.
   */
  let queue: Promise<void> = Promise.resolve();
  function enqueue(step: () => Promise<void>): Promise<void> {
    queue = queue.then(step, step);
    return queue;
  }

  function wanted(): string | null {
    return disposed ? null : recordId() || null;
  }

  function refuse(id: string, at: RefusedAt, e: unknown) {
    held.value = false;
    refused = { id, at };
    if (e instanceof BoundaryError) {
      message.value = e.message;
      heldBy.value = e.isBusy ? ((e.detail.holder as string) ?? null) : null;
    } else {
      message.value = `This could not be opened for editing: ${String(e)}`;
      heldBy.value = null;
    }
  }

  function clear() {
    held.value = false;
    pending.value = false;
    heldBy.value = null;
    message.value = null;
    refused = null;
  }

  /** Bring what the host holds for us into line with what the screen shows. */
  async function reconcile() {
    const want = wanted();

    if (holding && holding !== want) {
      const letGo = holding;
      holding = null;
      held.value = false;
      try {
        await api.leaseRelease(kind, letGo);
      } catch {
        // Letting go is best-effort: the hold expires on its own, and a dropped
        // connection releases it at the host.
      }
    }

    if (!want) {
      stopHeartbeat();
      clear();
      return;
    }
    if (holding === want) return;

    // A different record from the one last refused starts with a clean slate,
    // so one record's refusal is never shown on another.
    if (refused && refused.id !== want) clear();

    pending.value = true;
    try {
      await api.leaseAcquire(kind, want);
      holding = want;
      const wasRefused = refused;
      // The screen may have moved on while we waited. The hold is still ours
      // to give back, which the next queued step does; it is just not the one
      // on screen.
      if (wanted() === want) {
        held.value = true;
        heldBy.value = null;
        message.value = null;
        refused = null;
        if (wasRefused) options.onRegained?.(want, wasRefused.at);
      }
    } catch (e) {
      if (wanted() === want) refuse(want, 'acquire', e);
    } finally {
      pending.value = false;
    }
    startHeartbeat();
  }

  /** One beat: renew what we hold, or ask again for what we were refused. */
  async function beat() {
    const want = wanted();
    if (!want) return;
    if (holding !== want) {
      await reconcile();
      return;
    }
    try {
      await api.leaseRenew(kind, want);
    } catch (e) {
      // A refusal means someone took it while we were away. Say so now rather
      // than letting the person discover it when their save is refused. Any
      // other failure is a dropped beat, and the next one is well inside the
      // timeout.
      if (e instanceof BoundaryError && holding === want) {
        holding = null;
        refuse(want, 'renew', e);
      }
    }
  }

  function startHeartbeat() {
    if (timer || disposed) return;
    timer = setInterval(() => void enqueue(beat), HEARTBEAT_MS);
  }

  function stopHeartbeat() {
    if (timer) clearInterval(timer);
    timer = null;
  }

  // Following the id means a screen that switches records — or closes its
  // dialog — hands the old one back instead of sitting on it.
  watch(
    () => recordId() || null,
    () => void enqueue(reconcile),
    { immediate: true }
  );

  onScopeDispose(() => {
    disposed = true;
    stopHeartbeat();
    void enqueue(reconcile);
  });

  /** Ask again now, for a "Try again" button. Resolves once answered. */
  function retry() {
    return enqueue(reconcile);
  }

  return { held, pending, heldBy, message, retry };
}
