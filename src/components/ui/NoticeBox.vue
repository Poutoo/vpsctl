<script setup lang="ts">
/**
 * Avis encadré (planche VI) : icône, titre, explication et action
 * facultative à droite. Le ton est porté par la forme de l'icône et le
 * contour, jamais par la couleur seule.
 */
import { computed } from "vue";
import AppIcon from "./AppIcon.vue";
import StatusIcon from "./StatusIcon.vue";

const props = withDefaults(
  defineProps<{
    tone?: "info" | "success" | "warning";
    title: string;
  }>(),
  { tone: "info" },
);

const role = computed(() => (props.tone === "warning" ? "alert" : "status"));
</script>

<template>
  <div class="notice" :class="tone" :role="role">
    <StatusIcon v-if="tone === 'warning'" class="shape" name="triangle" />
    <AppIcon
      v-else
      class="shape"
      :name="tone === 'success' ? 'check-circle' : 'info'"
    />
    <div class="content">
      <p class="title">{{ title }}</p>
      <p class="text"><slot /></p>
    </div>
    <div v-if="$slots.action" class="action"><slot name="action" /></div>
  </div>
</template>

<style scoped>
.notice {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-4);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border-strong);
}

.notice.warning {
  border-color: var(--color-warning);
}

.notice.success {
  border-color: var(--color-success);
}

.shape {
  flex-shrink: 0;
  margin-top: var(--sp-1);
  color: var(--color-text-muted);
}

.warning .shape {
  color: var(--color-warning);
}

.success .shape {
  color: var(--color-success);
}

.content {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.title {
  margin: 0;
  font-size: var(--fs-md);
  font-weight: 600;
}

.text {
  margin: 0;
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.action {
  flex-shrink: 0;
  align-self: center;
}
</style>
