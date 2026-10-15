<script setup lang="ts">
/**
 * État vide ou d'erreur d'une zone de contenu, sur surface opaque :
 * étiquette, titre, explication et actions éventuelles. Une erreur est
 * signalée par le triangle d'alerte et un trait rouge, pas par la couleur seule.
 */
import StatusIcon from "./StatusIcon.vue";

withDefaults(
  defineProps<{
    tone?: "neutral" | "error";
    /** Étiquette en capitales, ex. « 0 conteneur · docker ps -a ». */
    kicker?: string;
    title?: string;
  }>(),
  { tone: "neutral", kicker: undefined, title: undefined },
);
</script>

<template>
  <div
    class="state"
    :class="tone"
    :role="tone === 'error' ? 'alert' : undefined"
  >
    <p v-if="kicker" class="caps kicker">
      <StatusIcon v-if="tone === 'error'" class="shape" name="triangle" />
      {{ kicker }}
    </p>
    <h3 v-if="title" class="title">{{ title }}</h3>
    <div class="text"><slot /></div>
    <div v-if="$slots.actions" class="actions"><slot name="actions" /></div>
  </div>
</template>

<style scoped>
.state {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding: var(--sp-5);
  background: var(--color-surface);
}

.error {
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-danger);
}

.kicker {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  color: var(--color-text-muted);
}

.error .kicker {
  color: var(--color-danger-text);
}

.shape {
  color: var(--color-danger);
}

.title {
  font-size: var(--fs-xl);
}

.text {
  max-width: 60ch;
  color: var(--color-text-muted);
  font-size: var(--fs-md);
}

.error .text {
  color: var(--color-text);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-3);
  margin-top: var(--sp-2);
}
</style>
