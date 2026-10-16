<script setup lang="ts">
/**
 * Fenêtre GO / NO GO. Le focus s'ouvre sur NO GO, Échap vaut NO GO, GO est à
 * droite et n'est jamais l'action par défaut. Avec `acknowledge`, GO reste
 * désactivé tant que la case n'est pas cochée.
 */
import {
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogOverlay,
  AlertDialogPortal,
  AlertDialogRoot,
  AlertDialogTitle,
} from "reka-ui";
import { computed, ref, watch } from "vue";
import { useConfirm } from "../../composables/useConfirm";
import AppButton from "./AppButton.vue";
import StatusIcon from "./StatusIcon.vue";

const { state, settle } = useConfirm();

const acknowledged = ref(false);
const noGo = ref<InstanceType<typeof AppButton> | null>(null);

watch(
  () => state.open,
  (open) => {
    if (open) acknowledged.value = false;
  },
);

const goDisabled = computed(
  () => Boolean(state.acknowledge) && !acknowledged.value,
);

// Fermeture externe (Échap) : vaut annulation. Les boutons n'utilisent pas
// AlertDialogAction / AlertDialogCancel : leur fermeture interne s'exécute
// avant notre @click et résolvait la confirmation à `false`.
function onOpenChange(open: boolean) {
  if (!open && state.resolve) settle(false);
}

/** Focus initial sur NO GO, même quand une case à cocher la précède. */
function focusNoGo(event: Event) {
  event.preventDefault();
  (noGo.value?.$el as HTMLElement | undefined)?.focus();
}
</script>

<template>
  <AlertDialogRoot :open="state.open" @update:open="onOpenChange">
    <AlertDialogPortal>
      <AlertDialogOverlay class="overlay" />
      <AlertDialogContent
        class="dialog"
        :class="{ danger: state.danger }"
        @open-auto-focus="focusNoGo"
      >
        <header class="banner">
          <span class="caps kicker">
            <StatusIcon name="triangle" />
            Confirmation requise
          </span>
          <span v-if="state.danger" class="caps aside">Irréversible</span>
        </header>

        <div class="body">
          <AlertDialogTitle class="title">{{ state.title }}</AlertDialogTitle>
          <p v-if="state.subject" class="subject">{{ state.subject }}</p>
          <AlertDialogDescription as="ul" class="consequences">
            <li v-for="check in state.checks" :key="check.label" class="check">
              <StatusIcon class="check-shape" name="check" />
              <span>
                <strong>{{ check.label }}</strong>
                <span v-if="check.detail" class="check-detail">{{
                  check.detail
                }}</span>
              </span>
            </li>
            <li v-for="item in state.consequences" :key="item">
              <span class="dash" aria-hidden="true">—</span>
              <span>{{ item }}</span>
            </li>
          </AlertDialogDescription>
          <p v-if="state.warning" class="warning" role="alert">
            <StatusIcon class="warning-shape" name="triangle" />
            <strong>{{ state.warning }}</strong>
          </p>
          <label v-if="state.acknowledge" class="acknowledge">
            <input v-model="acknowledged" type="checkbox" />
            {{ state.acknowledge }}
          </label>
        </div>

        <footer class="footer">
          <span class="caps hint">Échap = NO GO</span>
          <AppButton ref="noGo" @click="settle(false)">
            <span class="caps code">NO GO</span> Annuler
          </AppButton>
          <AppButton
            :variant="state.danger ? 'danger' : 'primary'"
            :disabled="goDisabled"
            @click="settle(true)"
          >
            <span class="caps code">GO</span> {{ state.confirmLabel }}
          </AppButton>
        </footer>
      </AlertDialogContent>
    </AlertDialogPortal>
  </AlertDialogRoot>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-dialog);
  background: var(--color-scrim);
  animation: fade-in var(--dur-slow) var(--ease-out);
}

.dialog {
  position: fixed;
  top: 50%;
  left: 50%;
  z-index: var(--z-dialog);
  width: min(calc(100vw - 2 * var(--sp-4)), calc(7 * var(--sp-8)));
  transform: translate(-50%, -50%);
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-2);
  box-shadow: var(--shadow-overlay);
  animation: fade-in var(--dur-slow) var(--ease-out);
}

@keyframes fade-in {
  from {
    opacity: 0;
  }
}

.banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
}

.kicker {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  color: var(--color-warning);
}

.danger .kicker {
  color: var(--color-danger-text);
}

.aside {
  color: var(--color-text-muted);
}

.body {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  padding: var(--sp-5) var(--sp-5) var(--sp-4);
}

.title {
  font-size: var(--fs-2xl);
}

.subject {
  color: var(--color-action);
  font-family: var(--font-mono);
  font-size: var(--fs-md);
  overflow-wrap: anywhere;
}

.consequences {
  margin: 0;
  padding: 0;
  list-style: none;
  border-top: var(--border-width) solid var(--color-border);
  font-size: var(--fs-md);
}

.consequences li {
  display: flex;
  gap: var(--sp-3);
  padding: var(--sp-2) 0;
  border-bottom: var(--border-width) solid var(--color-border);
}

.dash {
  color: var(--color-text-muted);
}

/* Un long chemin sans espace se coupe au lieu de sortir de la fenêtre. */
.consequences li > span:last-child,
.warning strong,
.check-detail {
  min-width: 0;
  overflow-wrap: anywhere;
}

/* Condition vérifiée : coche (forme) et libellé, puis la donnée brute. */
.check {
  align-items: center;
}

.check-shape {
  color: var(--color-success);
}

.check-detail {
  margin-left: var(--sp-2);
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

/* Avertissement : triangle (forme rouge) et trait à gauche, texte lisible. */
.warning {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-2);
  padding: var(--sp-2) var(--sp-3);
  background: var(--color-danger-tint);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-danger);
  color: var(--color-danger-text);
  font-size: var(--fs-md);
}

.warning-shape {
  flex-shrink: 0;
  margin-top: var(--sp-1);
  color: var(--color-danger);
}

.acknowledge {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-2);
  font-size: var(--fs-md);
  cursor: pointer;
}

.footer {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-4);
  background: var(--color-surface-raised);
  border-top: var(--border-width) solid var(--color-border);
}

.hint {
  margin-right: auto;
  color: var(--color-text-muted);
}

.code {
  font-weight: 600;
}
</style>
