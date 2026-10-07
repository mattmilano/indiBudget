/**
 * What an edit form needs to survive a refused save.
 *
 * A form remembers the version of the record it opened with and sends it back
 * with the save. If someone else saved in between, the host refuses rather than
 * letting one person's changes silently replace the other's. This keeps the
 * form open with everything the person typed, says what happened, and offers
 * the two honest ways forward: load the other person's version, or keep these
 * edits and knowingly save them over it.
 *
 * ```ts
 * const conflict = useSaveConflict({
 *   label: 'account',
 *   versionOf: (a) => a.row_version,
 *   fetchLatest: async () => { await store.fetchAccounts(); return store.accountsById[id]; },
 *   loadIntoForm: (a) => fillForm(a),
 * });
 * ```
 */
import { ref } from 'vue';
import { BoundaryError } from '../lib/rpc';

export type RefusalKind = 'stale' | 'deleted' | 'other';

/** The host says "That budget has been deleted." when the row is gone. */
export function isDeletedRefusal(e: unknown): boolean {
  return e instanceof BoundaryError && e.kind === 'invalid' && /has been deleted/.test(e.message);
}

export interface SaveConflictOptions<T> {
  /** How the record is named to a person: "account", "budget". */
  label: string;
  versionOf: (record: T) => number;
  /** Re-read the record from the host, or nothing if it no longer exists. */
  fetchLatest: () => Promise<T | undefined>;
  /** Put a record's saved values into the form, replacing what is there. */
  loadIntoForm: (record: T) => void;
}

export function useSaveConflict<T>(options: SaveConflictOptions<T>) {
  /** The version the form is working from, sent back with the save. */
  const version = ref<number | undefined>(undefined);
  /** The refusal sentence, shown in the form. */
  const sentence = ref<string | null>(null);
  const kind = ref<RefusalKind | null>(null);
  /** Shown once the person has chosen to keep their edits over a newer version. */
  const note = ref<string | null>(null);
  /** True while re-reading the record. */
  const reloading = ref(false);

  function clear() {
    sentence.value = null;
    kind.value = null;
    note.value = null;
  }

  /** The form has just opened on this record. */
  function begin(record: T) {
    version.value = options.versionOf(record);
    clear();
  }

  /** A save was refused. The form stays open; this says why. */
  function refused(e: unknown) {
    note.value = null;
    if (e instanceof BoundaryError) {
      sentence.value = e.message;
      kind.value = e.isStale ? 'stale' : isDeletedRefusal(e) ? 'deleted' : 'other';
    } else {
      sentence.value = `That could not be saved: ${String(e)}`;
      kind.value = 'other';
    }
  }

  function gone() {
    sentence.value =
      `This ${options.label} has been deleted by someone else, so there is no newer version to load.`;
    kind.value = 'deleted';
    note.value = null;
  }

  /** Replace the form with the latest saved version. */
  async function loadLatest() {
    reloading.value = true;
    try {
      const latest = await options.fetchLatest();
      if (latest === undefined) return gone();
      options.loadIntoForm(latest);
      version.value = options.versionOf(latest);
      clear();
    } finally {
      reloading.value = false;
    }
  }

  /**
   * Keep what the person typed, but work from the latest version.
   *
   * Only ever on an explicit choice, and the note that follows says plainly
   * that saving will now replace the other person's changes.
   */
  async function keepMine() {
    reloading.value = true;
    try {
      const latest = await options.fetchLatest();
      if (latest === undefined) return gone();
      version.value = options.versionOf(latest);
      clear();
      note.value =
        `Your edits are kept and now sit on top of the latest version of this ${options.label}. ` +
        'Saving will replace the other changes with yours.';
    } finally {
      reloading.value = false;
    }
  }

  return { version, sentence, kind, note, reloading, begin, refused, clear, loadLatest, keepMine };
}
