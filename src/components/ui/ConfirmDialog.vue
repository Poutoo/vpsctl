<script setup lang="ts">
import {
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogOverlay,
  AlertDialogPortal,
  AlertDialogRoot,
  AlertDialogTitle,
} from "reka-ui";
import { useConfirm } from "../../composables/useConfirm";

const { state, settle } = useConfirm();

// Fermeture externe (Échap) : vaut annulation. Les boutons n'utilisent pas
// AlertDialogAction / AlertDialogCancel : leur fermeture interne s'exécute
// avant notre @click et résolvait la confirmation à `false`.
function onOpenChange(open: boolean) {
  if (!open && state.resolve) settle(false);
}
</script>

<template>
  <AlertDialogRoot :open="state.open" @update:open="onOpenChange">
    <AlertDialogPortal>
      <AlertDialogOverlay class="overlay" />
      <AlertDialogContent class="dialog">
        <AlertDialogTitle class="title">{{ state.title }}</AlertDialogTitle>
        <AlertDialogDescription>{{ state.message }}</AlertDialogDescription>
        <div class="actions">
          <button type="button" @click="settle(false)">Annuler</button>
          <button
            type="button"
            :class="{ danger: state.danger }"
            @click="settle(true)"
          >
            {{ state.confirmLabel }}
          </button>
        </div>
      </AlertDialogContent>
    </AlertDialogPortal>
  </AlertDialogRoot>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: var(--color-bg);
  opacity: var(--overlay-opacity);
}

.dialog {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  max-width: calc(100vw - 2 * var(--space-4));
  padding: var(--space-5);
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
  border-radius: var(--radius-md);
}

.title {
  margin: 0 0 var(--space-3);
  font-size: var(--font-size-lg);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-3);
  margin-top: var(--space-5);
}

.danger {
  color: var(--color-danger);
}
</style>
