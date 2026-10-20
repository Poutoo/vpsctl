<script setup lang="ts">
/** Texte dont les correspondances avec le filtre sont surlignées. */
import { computed } from "vue";
import { segments } from "../../listFilter";

const props = defineProps<{ text: string; query: string }>();

const parts = computed(() => segments(props.text, props.query));
</script>

<template>
  <template v-for="(part, index) in parts" :key="index">
    <mark v-if="part.match" class="match">{{ part.text }}</mark>
    <template v-else>{{ part.text }}</template>
  </template>
</template>

<style scoped>
.match {
  background: var(--color-selection);
  color: inherit;
  text-decoration: underline;
  text-decoration-color: var(--color-action);
  text-underline-offset: var(--underline-offset);
}
</style>
