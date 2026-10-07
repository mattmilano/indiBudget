<script setup lang="ts">
/**
 * Why a save was refused, shown inside the form that was being saved.
 *
 * The form stays open behind this with everything the person typed. For a
 * record someone else changed in the meantime, it offers the two honest ways
 * forward rather than choosing one for them.
 */
import type { RefusalKind } from '../composables/useSaveConflict';

defineProps<{
  sentence: string | null;
  kind: RefusalKind | null;
  note: string | null;
  reloading: boolean;
}>();

defineEmits<{
  (e: 'load-latest'): void;
  (e: 'keep-mine'): void;
}>();
</script>

<template>
  <div
    v-if="sentence"
    role="alert"
    class="px-4 py-3 rounded-lg bg-amber-50 dark:bg-amber-900/30 text-amber-800 dark:text-amber-300 text-sm space-y-2"
  >
    <p>{{ sentence }}</p>
    <template v-if="kind === 'stale'">
      <p>
        What you typed is still here and has not been saved. Load their version to see what
        changed, or keep your edits and save them over it.
      </p>
      <div class="flex flex-wrap gap-2">
        <button
          type="button"
          :disabled="reloading"
          class="px-3 py-1.5 text-xs font-medium rounded-lg bg-amber-600 text-white hover:bg-amber-700 transition-colors disabled:opacity-50"
          @click="$emit('load-latest')"
        >
          {{ reloading ? 'Loading…' : 'Load their version' }}
        </button>
        <button
          type="button"
          :disabled="reloading"
          class="px-3 py-1.5 text-xs font-medium rounded-lg border border-amber-400 dark:border-amber-700 hover:bg-amber-100 dark:hover:bg-amber-900/50 transition-colors disabled:opacity-50"
          @click="$emit('keep-mine')"
        >
          Keep my edits
        </button>
      </div>
    </template>
    <p v-else-if="kind === 'deleted'">Nothing you typed here has been saved.</p>
  </div>
  <p
    v-else-if="note"
    role="status"
    class="px-4 py-3 rounded-lg bg-amber-50 dark:bg-amber-900/30 text-amber-800 dark:text-amber-300 text-sm"
  >
    {{ note }}
  </p>
</template>
