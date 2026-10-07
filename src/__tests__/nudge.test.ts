/**
 * The host's nudge, as the window experiences it.
 *
 * A nudge says only that something changed; the window must answer by
 * catching up straight away rather than at its next beat, one catch-up at a
 * time so the mark never moves backwards, and must stop listening once it is
 * no longer sharing.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

let nudge: (() => void) | null = null;
const unlisten = vi.fn(() => {
  nudge = null;
});
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (_name: string, handler: () => void) => {
    nudge = handler;
    return unlisten;
  }),
}));

let seq = 0;
let release: (() => void) | null = null;
const newsCatchUp = vi.fn(async () => {
  // Held open on request, to see what happens to a nudge mid-way.
  if (holdNext) {
    holdNext = false;
    await new Promise<void>((r) => (release = r));
  }
  seq += 1;
  return { status: 'notices', notices: [], mark: { run: 'r', seq } };
});
let holdNext = false;
vi.mock('../services/api', () => ({
  newsCatchUp: (...a: unknown[]) => newsCatchUp(...(a as [])),
}));

const { useMultiUserStore, NEWS_BEAT_MS } = await import('../stores/multiuser');

async function settle() {
  await vi.advanceTimersByTimeAsync(0);
}

describe('push nudges', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    setActivePinia(createPinia());
    newsCatchUp.mockClear();
    unlisten.mockClear();
    nudge = null;
    seq = 0;
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('catches up at once when nudged, without waiting for the beat', async () => {
    const store = useMultiUserStore();
    store.startBeat();
    await settle();
    expect(newsCatchUp).toHaveBeenCalledTimes(1);
    expect(nudge).not.toBeNull();

    nudge!();
    await settle();
    expect(newsCatchUp).toHaveBeenCalledTimes(2);

    // The beat still runs as the fallback.
    await vi.advanceTimersByTimeAsync(NEWS_BEAT_MS);
    expect(newsCatchUp).toHaveBeenCalledTimes(3);
    store.stopBeat();
  });

  it('folds nudges that arrive mid-way into one more catch-up, never two at once', async () => {
    const store = useMultiUserStore();
    store.startBeat();
    await settle();
    newsCatchUp.mockClear();

    holdNext = true;
    nudge!();
    await settle();
    nudge!();
    nudge!();
    await settle();
    expect(newsCatchUp).toHaveBeenCalledTimes(1); // still in flight

    release!();
    await settle();
    expect(newsCatchUp).toHaveBeenCalledTimes(2); // exactly one more
    store.stopBeat();
  });

  it('stops listening when sharing stops', async () => {
    const store = useMultiUserStore();
    store.startBeat();
    await settle();
    store.stopBeat();
    expect(unlisten).toHaveBeenCalled();
    expect(nudge).toBeNull();
  });
});
