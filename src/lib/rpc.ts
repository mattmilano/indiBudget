/**
 * The one door.
 *
 * Every backend call in the app goes through here. When this machine is
 * connected to someone else's budget the call travels to that host; otherwise
 * it is dispatched locally against the same registry, with the same edit holds
 * and the same news. One place decides what is local and what travels, so no
 * screen has to know the difference.
 *
 * `api.ts` imports `invoke` from this module instead of from Tauri, which is
 * why none of the seventy call sites needed to change.
 */
import { invoke as tauriInvoke } from '@tauri-apps/api/core';

export interface BoundaryOk {
  status: 'ok';
  value: unknown;
}

export interface BoundaryErr {
  status: 'err';
  error: { kind: string; [k: string]: unknown };
  sentence: string;
}

export type BoundaryResponse = BoundaryOk | BoundaryErr;

/** Raised when the boundary refuses, carrying the sentence to show a person. */
export class BoundaryError extends Error {
  readonly kind: string;
  readonly detail: Record<string, unknown>;

  constructor(error: BoundaryErr) {
    super(error.sentence);
    this.name = 'BoundaryError';
    this.kind = error.error.kind;
    this.detail = error.error;
  }

  /** Someone else holds the edit hold on this record. */
  get isBusy() {
    return this.kind === 'busy';
  }

  /** The row moved under us; the screen should reopen it. */
  get isStale() {
    return this.kind === 'stale';
  }

  /** The budget is closed for maintenance. */
  get isMaintenance() {
    return this.kind === 'maintenance';
  }

  /** This person lacks the grant. */
  get isDenied() {
    return this.kind === 'denied';
  }
}

/**
 * Whether this machine is viewing someone else's budget.
 *
 * Kept here rather than read from the store to avoid a circular import — the
 * store is what sets it.
 */
let connectedToHost = false;

export function setConnectedToHost(connected: boolean) {
  connectedToHost = connected;
}

export function isConnectedToHost() {
  return connectedToHost;
}

/**
 * Drop-in replacement for Tauri's `invoke`.
 *
 * A command the registry does not know is host-only — file dialogs, backups to
 * a path, the encryption session. Those run directly against this machine, but
 * only when this machine is the one holding the data. While connected to
 * someone else's budget they are refused rather than silently acting on the
 * local database, which is not the one on screen.
 */
export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const response = await tauriInvoke<BoundaryResponse>('boundary_invoke', {
    command,
    args: args ?? null,
  });

  if (response.status === 'ok') {
    return response.value as T;
  }

  if (response.error.kind === 'unknown_command') {
    if (connectedToHost) {
      throw new BoundaryError({
        ...response,
        sentence: `"${command}" can only be done on the computer hosting this budget.`,
      });
    }
    return tauriInvoke<T>(command, args);
  }

  throw new BoundaryError(response);
}

/** Call a host-only command directly, bypassing the boundary. */
export function invokeLocal<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return tauriInvoke<T>(command, args);
}
