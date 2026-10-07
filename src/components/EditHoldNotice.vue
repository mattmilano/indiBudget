<script setup lang="ts">
/**
 * Says who is editing a record, at the top of its form, before anyone types.
 *
 * The form beneath is locked while this shows. The edit hold keeps asking on
 * its heartbeat, so the form opens by itself once the other person finishes;
 * the button is for someone who would rather not wait for the next beat.
 */
defineProps<{
  message: string | null;
  heldBy: string | null;
  pending: boolean;
}>();

defineEmits<{
  (e: 'retry'): void;
}>();
</script>

<template>
  <div
    v-if="message"
    role="status"
    class="px-4 py-3 rounded-lg bg-amber-50 dark:bg-amber-900/30 text-amber-800 dark:text-amber-300 text-sm space-y-2"
  >
    <p class="font-medium">{{ message }}</p>
    <p v-if="heldBy">
      You can look, but changes are paused until {{ heldBy }} has finished. This form opens up by
      itself when it is free.
    </p>
    <button
      type="button"
      :disabled="pending"
      class="px-3 py-1.5 text-xs font-medium rounded-lg border border-amber-400 dark:border-amber-700 hover:bg-amber-100 dark:hover:bg-amber-900/50 transition-colors disabled:opacity-50"
      @click="$emit('retry')"
    >
      {{ pending ? 'Checking…' : 'Try again' }}
    </button>
  </div>
</template>
