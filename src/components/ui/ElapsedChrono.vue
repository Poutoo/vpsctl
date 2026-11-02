<script setup lang="ts">
/** Chrono « T+12 » depuis `since` (horodatage `Date.now()`), à la seconde. */
import { toRef } from "vue";
import { formatElapsed, useElapsed } from "../../composables/useElapsed";

const props = defineProps<{ since: number }>();

const elapsed = useElapsed(toRef(props, "since"), {
  clock: () => Date.now(),
  tickMs: 1_000,
});
</script>

<template>
  <span class="chrono">{{
    elapsed === null ? "" : formatElapsed(elapsed, "s")
  }}</span>
</template>

<style scoped>
.chrono {
  flex-shrink: 0;
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}
</style>
