/**
 * The edit hold as a screen experiences it.
 *
 * The Rust side proves who may hold what. This proves the screen asks at the
 * right moments: that closing a dialog hands the hold back, that reopening the
 * same record takes it again, that a refusal names who is editing, and that a
 * hold being granted after the dialog closed is not left behind renewing
 * itself on a record nobody has open.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { effectScope, ref } from 'vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const leaseAcquire = vi.fn();
const leaseRenew = vi.fn();
const leaseRelease = vi.fn();
vi.mock('../services/api', () => ({
  leaseAcquire: (...a: unknown[]) => leaseAcquire(...a),
  leaseRenew: (...a: unknown[]) => leaseRenew(...a),
  leaseRelease: (...a: unknown[]) => leaseRelease(...a),
}));

const { BoundaryError } = await import('../lib/rpc');
const { useLease, HEARTBEAT_MS } = await import('../composables/useLease');

function busy(holder: string) {
  return new BoundaryError({
    status: 'err',
    error: { kind: 'busy', record: 'budget', holder },
    sentence: `${holder} is editing this budget right now.`,
  });
}

/** Let queued watchers and promise chains run to completion. */
async function settle() {
  await vi.advanceTimersByTimeAsync(0);
}

/** Every call made to the host, in order, as `verb:id`. */
function calls() {
  const all = [
    ...leaseAcquire.mock.calls.map((c, i) => ({ v: 'acquire', id: c[1], o: leaseAcquire.mock.invocationCallOrder[i] })),
    ...leaseRenew.mock.calls.map((c, i) => ({ v: 'renew', id: c[1], o: leaseRenew.mock.invocationCallOrder[i] })),
    ...leaseRelease.mock.calls.map((c, i) => ({ v: 'release', id: c[1], o: leaseRelease.mock.invocationCallOrder[i] })),
  ];
  return all.sort((a, b) => a.o - b.o).map((c) => `${c.v}:${c.id}`);
}

function mount(initial: string | null, options = {}) {
  const id = ref<string | null>(initial);
  const scope = effectScope();
  const lease = scope.run(() => useLease('budget', () => id.value, options))!;
  return { id, scope, lease };
}

beforeEach(() => {
  vi.useFakeTimers();
  leaseAcquire.mockReset().mockResolvedValue({ held: true });
  leaseRenew.mockReset().mockResolvedValue({ held: true });
  leaseRelease.mockReset().mockResolvedValue({ held: false });
});

afterEach(() => {
  vi.useRealTimers();
});

describe('useLease', () => {
  it('takes the hold when an edit opens, and nothing when none is open', async () => {
    const closed = mount(null);
    await settle();
    expect(leaseAcquire).not.toHaveBeenCalled();
    expect(closed.lease.held.value).toBe(false);

    const open = mount('b1');
    await settle();
    expect(calls()).toEqual(['acquire:b1']);
    expect(open.lease.held.value).toBe(true);
    expect(open.lease.message.value).toBeNull();
  });

  it('closing and reopening the same record releases it and takes it again', async () => {
    const { id, lease } = mount('b1');
    await settle();

    id.value = null;
    await settle();
    expect(lease.held.value).toBe(false);

    id.value = 'b1';
    await settle();
    expect(lease.held.value).toBe(true);
    expect(calls()).toEqual(['acquire:b1', 'release:b1', 'acquire:b1']);
  });

  it('a release still in flight is not overtaken by the reopen', async () => {
    const { id, lease } = mount('b1');
    await settle();

    let finishRelease!: () => void;
    leaseRelease.mockImplementationOnce(
      () => new Promise((resolve) => (finishRelease = () => resolve({ held: false })))
    );

    id.value = null;
    await settle();
    id.value = 'b1';
    await settle();
    // The reopen waits for the release, so the host cannot see them reversed.
    expect(calls()).toEqual(['acquire:b1', 'release:b1']);

    finishRelease();
    await settle();
    expect(calls()).toEqual(['acquire:b1', 'release:b1', 'acquire:b1']);
    expect(lease.held.value).toBe(true);
  });

  it('a hold granted after the dialog closed is handed straight back, not kept renewing', async () => {
    let grant!: () => void;
    leaseAcquire.mockImplementationOnce(
      () => new Promise((resolve) => (grant = () => resolve({ held: true })))
    );
    const { id, lease } = mount('b1');
    await settle();

    id.value = null;
    await settle();
    grant();
    await settle();

    expect(lease.held.value).toBe(false);
    expect(calls()).toEqual(['acquire:b1', 'release:b1']);

    await vi.advanceTimersByTimeAsync(HEARTBEAT_MS * 3);
    expect(leaseRenew).not.toHaveBeenCalled();
  });

  it('a busy refusal names who is editing and keeps the form locked', async () => {
    leaseAcquire.mockRejectedValueOnce(busy('Sam'));
    const { lease } = mount('b1');
    await settle();

    expect(lease.held.value).toBe(false);
    expect(lease.heldBy.value).toBe('Sam');
    expect(lease.message.value).toBe('Sam is editing this budget right now.');
  });

  it('asks again on the heartbeat and says so once the record is free', async () => {
    const onRegained = vi.fn();
    leaseAcquire.mockRejectedValueOnce(busy('Sam'));
    const { lease } = mount('b1', { onRegained });
    await settle();
    expect(lease.held.value).toBe(false);

    await vi.advanceTimersByTimeAsync(HEARTBEAT_MS);
    expect(lease.held.value).toBe(true);
    expect(lease.message.value).toBeNull();
    expect(lease.heldBy.value).toBeNull();
    expect(onRegained).toHaveBeenCalledWith('b1', 'acquire');
  });

  it('a renewal refused because someone took it locks the form and names them', async () => {
    const onRegained = vi.fn();
    const { lease } = mount('b1', { onRegained });
    await settle();
    expect(lease.held.value).toBe(true);

    leaseRenew.mockRejectedValueOnce(busy('Alex'));
    await vi.advanceTimersByTimeAsync(HEARTBEAT_MS);
    expect(lease.held.value).toBe(false);
    expect(lease.heldBy.value).toBe('Alex');

    // When it comes back, the screen is told it was lost part-way through,
    // so it knows to leave what the person typed alone.
    await vi.advanceTimersByTimeAsync(HEARTBEAT_MS);
    expect(lease.held.value).toBe(true);
    expect(onRegained).toHaveBeenCalledWith('b1', 'renew');
  });

  it('a dropped heartbeat that is not a refusal keeps the hold', async () => {
    const { lease } = mount('b1');
    await settle();
    leaseRenew.mockRejectedValueOnce(new Error('network blip'));
    await vi.advanceTimersByTimeAsync(HEARTBEAT_MS);
    expect(lease.held.value).toBe(true);
    expect(lease.message.value).toBeNull();
  });

  it("one record's refusal is not shown on the next", async () => {
    leaseAcquire.mockRejectedValueOnce(busy('Sam'));
    const { id, lease } = mount('b1');
    await settle();
    expect(lease.message.value).not.toBeNull();

    id.value = null;
    await settle();
    expect(lease.message.value).toBeNull();

    id.value = 'b2';
    await settle();
    expect(lease.held.value).toBe(true);
    expect(lease.message.value).toBeNull();
    // b1 was never held, so there was nothing to give back.
    expect(leaseRelease).not.toHaveBeenCalled();
  });

  it('switching records hands the old one back before taking the new', async () => {
    const { id } = mount('b1');
    await settle();
    id.value = 'b2';
    await settle();
    expect(calls()).toEqual(['acquire:b1', 'release:b1', 'acquire:b2']);
  });

  it('closing the screen releases the hold and stops the heartbeat', async () => {
    const { scope } = mount('b1');
    await settle();
    scope.stop();
    await settle();
    expect(calls()).toEqual(['acquire:b1', 'release:b1']);

    await vi.advanceTimersByTimeAsync(HEARTBEAT_MS * 3);
    expect(leaseRenew).not.toHaveBeenCalled();
  });
});
