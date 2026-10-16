<script setup lang="ts">
/**
 * Pile de notifications, en haut à droite (`useNotify`). Succès : cercle,
 * fermé après 5 s ; erreur : carré, persistante. La pile s'arrête au-dessus
 * de la hauteur de la console ancrée : elle ne la recouvre jamais.
 */
import { useNotify, type Notification } from "../../composables/useNotify";
import AppIcon from "./AppIcon.vue";

const { items, dismiss } = useNotify();

function runAction(item: Notification) {
  item.action?.run();
  dismiss(item.id);
}
</script>

<template>
  <div class="host">
    <TransitionGroup name="toast" tag="ul" class="stack">
      <li
        v-for="item in items"
        :key="item.id"
        class="toast"
        :class="item.tone"
        :role="item.tone === 'error' ? 'alert' : 'status'"
      >
        <AppIcon
          class="badge"
          :name="item.tone === 'error' ? 'failure' : 'success'"
        />
        <div class="content">
          <p class="caps kicker">
            {{ item.tone === "error" ? "Erreur" : "Succès" }}
          </p>
          <p class="title">{{ item.title }}</p>
          <p v-if="item.detail" class="detail">{{ item.detail }}</p>
          <button
            v-if="item.action"
            type="button"
            class="action"
            @click="runAction(item)"
          >
            {{ item.action.label }}
            <AppIcon name="arrow-right" />
          </button>
        </div>
        <button
          type="button"
          class="close"
          aria-label="Fermer la notification"
          @click="dismiss(item.id)"
        >
          <AppIcon name="close" />
        </button>
      </li>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.host {
  position: fixed;
  top: calc(var(--titlebar-height) + var(--sp-4));
  right: var(--sp-4);
  /* Jamais sur la console ancrée, qui occupe le bas de la fenêtre. */
  bottom: calc(var(--console-dock-height) + var(--sp-7));
  z-index: var(--z-toast);
  width: var(--toast-width);
  max-width: calc(100vw - 2 * var(--sp-4));
  overflow: hidden;
  pointer-events: none;
}

.stack {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.toast {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-3) var(--sp-3) var(--sp-4);
  background: var(--color-surface-raised);
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-2);
  box-shadow: var(--shadow-overlay);
  pointer-events: auto;
}

.toast.error {
  border-color: var(--color-danger);
}

.badge {
  flex-shrink: 0;
  margin-top: var(--sp-1);
  font-size: var(--fs-xl);
}

.success .badge {
  color: var(--color-success);
}

.error .badge {
  color: var(--color-danger);
}

.content {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.success .kicker {
  color: var(--color-success);
}

.error .kicker {
  color: var(--color-danger-text);
}

.title {
  font-size: var(--fs-md);
  font-weight: 600;
  line-height: var(--lh-title);
}

.detail {
  overflow: hidden;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.action {
  display: inline-flex;
  align-items: center;
  align-self: flex-start;
  gap: var(--sp-1);
  margin-top: var(--sp-1);
  padding: 0;
  background: none;
  border: none;
  color: var(--color-action);
  font-size: var(--fs-sm);
  font-weight: 600;
  cursor: pointer;
}

.action:hover {
  color: var(--color-action-hover);
}

.close {
  display: inline-flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  width: var(--icon-button-size);
  height: var(--icon-button-size);
  padding: 0;
  background: none;
  border: none;
  border-radius: var(--r-1);
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
  cursor: pointer;
}

.close:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.toast-enter-active,
.toast-leave-active {
  transition:
    opacity var(--dur-base) var(--ease-out),
    transform var(--dur-base) var(--ease-out);
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateX(var(--sp-4));
}
</style>
