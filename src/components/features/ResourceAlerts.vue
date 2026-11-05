<script setup lang="ts">
/**
 * Bandeaux d'alerte (planche VII), par ordre de priorité : disque, mémoire,
 * charge. Seule l'alerte mémoire peut désigner un conteneur (« Voir … ») ;
 * l'alerte disque ne renvoie jamais vers un conteneur.
 */
import type { ContainerUsage } from "../../bindings/ContainerUsage";
import type { ResourceAlert } from "../../resourceText";
import AppButton from "../ui/AppButton.vue";
import AppIcon from "../ui/AppIcon.vue";
import StatusIcon from "../ui/StatusIcon.vue";

defineProps<{
  alerts: readonly ResourceAlert[];
  /** Liaison perdue : valeurs figées. */
  frozen: boolean;
}>();

defineEmits<{ show: [container: ContainerUsage] }>();
</script>

<template>
  <div v-if="alerts.length > 0" class="alerts">
    <div
      v-for="alert in alerts"
      :key="alert.resource"
      class="alert"
      :class="alert.level"
      role="alert"
      :data-testid="`alert-${alert.resource}`"
    >
      <StatusIcon
        class="shape"
        :name="alert.level === 'critical' ? 'alert' : 'triangle'"
      />
      <div class="content">
        <p class="title">
          <span class="caps level">{{
            alert.level === "critical" ? "Critique" : "Attention"
          }}</span>
          <strong data-testid="alert-title">{{ alert.title }}</strong>
        </p>
        <p class="detail">
          {{ alert.detail }}
          <template v-if="frozen">
            Valeur figée : le serveur ne répond plus.</template
          >
        </p>
      </div>
      <AppButton
        v-if="alert.container"
        class="show"
        data-testid="alert-show-container"
        @click="$emit('show', alert.container)"
      >
        Voir {{ alert.container.name }}
        <AppIcon name="arrow-right" />
      </AppButton>
    </div>
  </div>
</template>

<style scoped>
.alerts {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}

.alert {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-4);
  border: var(--border-width) solid var(--color-warning);
  background: var(--color-selection);
}

.alert.critical {
  border-color: var(--color-danger);
  background: var(--color-danger-tint);
}

.shape {
  flex-shrink: 0;
  font-size: var(--fs-xl);
  color: var(--color-warning);
}

.critical .shape {
  color: var(--color-danger-text);
}

.content {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.title {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--sp-3);
}

.level {
  color: var(--color-warning);
}

.critical .level {
  color: var(--color-danger-text);
}

.detail {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.show {
  flex-shrink: 0;
}
</style>
