<script setup lang="ts">
/**
 * Filtre sans résultat : une seule ligne dans le corps du tableau, en-têtes
 * conservés. Distinct de l'état vide (liste réellement vide).
 */
import AppButton from "./AppButton.vue";

defineProps<{
  /** Éléments de la liste complète. */
  total: number;
  query: string;
}>();

const emit = defineEmits<{ clear: [] }>();
</script>

<template>
  <div class="no-match" role="status">
    <span class="caps count">0 sur {{ total }}</span>
    <span class="text"
      >Aucun résultat pour «
      <span class="mono">{{ query.trim() }}</span> ».</span
    >
    <AppButton size="compact" @click="emit('clear')"
      >Effacer le filtre</AppButton
    >
  </div>
</template>

<style scoped>
.no-match {
  display: flex;
  align-items: center;
  gap: var(--sp-4);
  min-height: var(--row-height);
  padding: var(--sp-2) 0;
  font-family: var(--font-ui);
  font-size: var(--fs-md);
}

.count {
  color: var(--color-text-muted);
}

.text {
  min-width: 0;
  overflow-wrap: anywhere;
}
</style>
