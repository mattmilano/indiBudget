/**
 * The frontend half of the routing contract tested in Rust by
 * `src-tauri/tests/startup.rs`.
 *
 * `invoke` sends everything to `boundary_invoke`. A command the registry does
 * not know is host-only, and runs directly — unless this machine is connected
 * to someone else's budget, in which case running it here would act on the
 * wrong database, so it is refused.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const tauriInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => tauriInvoke(...a) }));

const { invoke, setConnectedToHost, BoundaryError } = await import('../lib/rpc');

const unknown = (command: string) => ({
  status: 'err',
  error: { kind: 'unknown_command', command },
  sentence: `"${command}" can only be run on the computer hosting this budget.`,
});

beforeEach(() => {
  tauriInvoke.mockReset();
  setConnectedToHost(false);
});

describe('rpc', () => {
  it('returns the value of a registered command', async () => {
    tauriInvoke.mockResolvedValueOnce({ status: 'ok', value: [{ id: 'a1' }] });
    await expect(invoke('get_accounts')).resolves.toEqual([{ id: 'a1' }]);
    expect(tauriInvoke).toHaveBeenCalledWith('boundary_invoke', { command: 'get_accounts', args: null });
  });

  it('runs a host-only command directly — which is how the database gets opened', async () => {
    tauriInvoke.mockResolvedValueOnce(unknown('init_app')).mockResolvedValueOnce(undefined);
    await invoke('init_app');
    expect(tauriInvoke).toHaveBeenNthCalledWith(2, 'init_app', undefined);
  });

  it('passes arguments through unchanged to the direct call', async () => {
    tauriInvoke.mockResolvedValueOnce(unknown('set_setting')).mockResolvedValueOnce(undefined);
    await invoke('set_setting', { key: 'user_agreement_accepted', value: 'true' });
    expect(tauriInvoke).toHaveBeenNthCalledWith(2, 'set_setting', {
      key: 'user_agreement_accepted',
      value: 'true',
    });
  });

  it('refuses a host-only command while connected to another computer', async () => {
    setConnectedToHost(true);
    tauriInvoke.mockResolvedValueOnce(unknown('export_backup'));
    await expect(invoke('export_backup', { path: '/x' })).rejects.toBeInstanceOf(BoundaryError);
    expect(tauriInvoke).toHaveBeenCalledTimes(1); // never ran locally
  });

  it('surfaces a refusal with its sentence rather than resolving', async () => {
    tauriInvoke.mockResolvedValueOnce({
      status: 'err',
      error: { kind: 'busy', record: 'budget', holder: 'Sam' },
      sentence: 'Sam is editing this budget right now.',
    });
    const err = (await invoke('lease_acquire').catch((e) => e)) as InstanceType<typeof BoundaryError>;
    expect(err).toBeInstanceOf(BoundaryError);
    expect(err.message).toContain('Sam');
    expect(err.isBusy).toBe(true);
  });

  it('lets a failure inside boundary_invoke itself propagate instead of swallowing it', async () => {
    tauriInvoke.mockRejectedValueOnce('Database not initialized');
    await expect(invoke('get_accounts')).rejects.toBe('Database not initialized');
  });
});
