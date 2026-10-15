<script setup lang="ts">
/**
 * Panneau opaque : bandeau d'en-tête (étiquette à gauche, complément à
 * droite), contenu, pied d'actions facultatif.
 */
withDefaults(
  defineProps<{
    tag?: "section" | "div" | "form" | "article";
    /** Teinte de l'étiquette du bandeau. */
    tone?: "neutral" | "warning" | "danger";
  }>(),
  { tag: "section", tone: "neutral" },
);
</script>

<template>
  <component :is="tag" class="panel">
    <header v-if="$slots.kicker || $slots.aside" class="banner">
      <span class="caps kicker" :class="tone"><slot name="kicker" /></span>
      <span v-if="$slots.aside" class="aside"><slot name="aside" /></span>
    </header>
    <div class="body"><slot /></div>
    <footer v-if="$slots.footer" class="footer"><slot name="footer" /></footer>
  </component>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  min-width: 0;
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
  border-radius: var(--r-0);
}

.banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  min-height: var(--row-height);
  padding: var(--sp-2) var(--sp-4);
  background: var(--color-surface-raised);
  border-bottom: var(--border-width) solid var(--color-border);
}

.kicker {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  min-width: 0;
  color: var(--color-text-muted);
}

.kicker.warning {
  color: var(--color-warning);
}

.kicker.danger {
  color: var(--color-danger-text);
}

.aside {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  min-width: 0;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

.body {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--sp-4);
  min-height: 0;
  padding: var(--sp-5);
}

.footer {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-5);
  background: var(--color-surface-raised);
  border-top: var(--border-width) solid var(--color-border);
}
</style>
