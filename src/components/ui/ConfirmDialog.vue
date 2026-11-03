<script setup lang="ts">
/**
 * Fenêtre GO / NO GO. Le focus s'ouvre sur NO GO, Échap vaut NO GO, GO est à
 * droite et n'est jamais l'action par défaut. Avec `acknowledge`, GO reste
 * désactivé tant que la case n'est pas cochée ; elle est décochée à chaque
 * ouverture. Un second choix (`alternative`) se place entre NO GO et GO.
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
  if (!open && state.resolve) settle("nogo");
}

const itemShapes = {
  ok: "check",
  warn: "bang",
  danger: "triangle",
  neutral: "dash",
} as const;

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
        :class="{
          danger: state.danger,
          wide: state.items?.length || state.acknowledgeHint || state.alert,
          widest: state.alternative,
        }"
        @open-auto-focus="focusNoGo"
      >
        <header class="banner">
          <span class="caps kicker">
            <StatusIcon name="triangle" />
            {{ state.kicker ?? "Confirmation requise" }}
          </span>
          <span
            v-if="state.aside"
            class="caps aside"
            data-testid="confirm-aside"
            >{{ state.aside }}</span
          >
          <span
            v-else-if="state.danger"
            class="caps aside"
            data-testid="confirm-aside"
            >Irréversible</span
          >
        </header>

        <div class="body">
          <AlertDialogTitle class="title">{{ state.title }}</AlertDialogTitle>
          <p v-if="state.subject" class="subject" data-testid="confirm-subject">
            {{ state.subject }}
          </p>
          <p v-if="state.command" class="command" data-testid="confirm-command">
            {{ state.command }}
          </p>
          <p v-if="state.alert" class="alert" data-testid="confirm-alert">
            <span class="caps alert-label">{{ state.alert.label }} ·</span>
            {{ state.alert.text }}
          </p>
          <AlertDialogDescription as="ul" class="consequences">
            <li
              v-for="removal in state.removals"
              :key="removal.label"
              class="check"
              data-testid="confirm-removal"
            >
              <StatusIcon class="removal-shape" name="bang" />
              <span>
                <strong>{{ removal.label }}</strong>
                <span v-if="removal.detail" class="check-detail">{{
                  removal.detail
                }}</span>
              </span>
            </li>
            <li
              v-for="check in state.checks"
              :key="check.label"
              class="check"
              data-testid="confirm-check"
            >
              <StatusIcon class="check-shape" name="check" />
              <span>
                <strong>{{ check.label }}</strong>
                <span v-if="check.detail" class="check-detail">{{
                  check.detail
                }}</span>
              </span>
            </li>
            <li
              v-for="item in state.items"
              :key="item.label"
              class="item"
              :class="item.tone"
              :data-testid="`confirm-item-${item.tone}`"
            >
              <StatusIcon class="item-shape" :name="itemShapes[item.tone]" />
              <span class="item-text">
                <strong>{{ item.label }}</strong>
                <span v-if="item.detail" class="item-detail">{{
                  item.detail
                }}</span>
              </span>
            </li>
            <li
              v-for="item in state.consequences"
              :key="typeof item === 'string' ? item : item.label"
              data-testid="confirm-consequence"
            >
              <span class="dash" aria-hidden="true">—</span>
              <span v-if="typeof item === 'string'">{{ item }}</span>
              <span v-else class="cited">
                {{ item.label }} :
                <pre class="cited-code">{{ item.code }}</pre>
              </span>
            </li>
          </AlertDialogDescription>
          <p
            v-if="state.warning"
            class="warning"
            role="alert"
            data-testid="confirm-warning"
          >
            <StatusIcon class="warning-shape" name="triangle" />
            <strong>{{ state.warning }}</strong>
          </p>
          <button
            v-if="state.link"
            type="button"
            class="link"
            data-testid="confirm-link"
            @click="settle('link')"
          >
            {{ state.link }}
          </button>
          <label
            v-if="state.acknowledge"
            class="acknowledge"
            :class="{
              boxed: state.acknowledgeHint,
              danger: state.acknowledgeDanger,
            }"
          >
            <input
              v-model="acknowledged"
              type="checkbox"
              data-testid="confirm-acknowledge"
            />
            <span class="acknowledge-text">
              <strong v-if="state.acknowledgeHint">{{
                state.acknowledge
              }}</strong>
              <template v-else>{{ state.acknowledge }}</template>
              <span v-if="state.acknowledgeHint" class="acknowledge-hint">{{
                state.acknowledgeHint
              }}</span>
            </span>
          </label>
        </div>

        <footer class="footer">
          <span class="caps hint">Échap = NO GO</span>
          <AppButton
            ref="noGo"
            data-testid="confirm-nogo"
            @click="settle('nogo')"
          >
            <span class="caps code">NO GO</span>
            {{ state.cancelLabel ?? "Annuler" }}
          </AppButton>
          <AppButton
            v-if="state.alternative"
            data-testid="confirm-alternative"
            @click="settle('alternative')"
          >
            {{ state.alternative }}
          </AppButton>
          <AppButton
            :variant="state.danger ? 'danger' : 'primary'"
            :disabled="goDisabled"
            data-testid="confirm-go"
            @click="settle('go')"
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

/* Constats, encadré ou case détaillée : fenêtre plus large ; trois boutons
   encore plus, le rappel « Échap = NO GO » sur sa propre ligne. */
.dialog.wide {
  width: min(calc(100vw - 2 * var(--sp-4)), calc(9 * var(--sp-8)));
}

.dialog.widest {
  width: min(calc(100vw - 2 * var(--sp-4)), calc(10 * var(--sp-8)));
}

.dialog.widest .hint {
  flex-basis: 100%;
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

.command {
  margin-top: calc(-1 * var(--sp-2));
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  overflow-wrap: anywhere;
}

.removal-shape {
  flex-shrink: 0;
  color: var(--color-warning);
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

/* Code cité (commande d'un script) : tel quel, lignes comprises, et défilant. */
.cited {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--sp-1);
}

/* Classe distincte de `.code`, l'étiquette GO / NO GO des boutons. */
.cited-code {
  max-height: calc(8 * var(--console-line-height) + 2 * var(--sp-2));
  margin: 0;
  overflow: auto;
  padding: var(--sp-2) var(--sp-3);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border);
  border-radius: var(--r-1);
  font-family: var(--font-mono);
  font-size: var(--fs-data);
  line-height: var(--console-line-height);
  white-space: pre;
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

/* Case détaillée : encadrée, rouge quand l'accès est menacé. */
.acknowledge.boxed {
  padding: var(--sp-3);
  border: var(--border-width) solid var(--color-border-strong);
}

.acknowledge.danger {
  border-color: var(--color-danger);
}

.acknowledge-text {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
}

.acknowledge-hint {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

/* Encadré d'alerte avant la liste : trait et fond rouges, texte lisible. */
.alert {
  margin: 0;
  padding: var(--sp-2) var(--sp-3);
  background: var(--color-danger-tint);
  border: var(--border-width) solid var(--color-danger);
  font-size: var(--fs-md);
}

.alert-label {
  color: var(--color-danger-text);
}

/* Constat : forme selon le ton, libellé, détail en dessous. */
.item-shape {
  flex-shrink: 0;
  margin-top: var(--sp-1);
}

.item.ok .item-shape {
  color: var(--color-success);
}

.item.warn .item-shape {
  color: var(--color-warning);
}

.item.danger .item-shape {
  color: var(--color-danger);
}

.item.neutral .item-shape {
  color: var(--color-text-muted);
}

.item-text {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
}

.item-detail {
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  white-space: pre-line;
}

.link {
  align-self: flex-start;
  padding: 0;
  background: none;
  border: none;
  color: var(--color-action);
  font: inherit;
  font-size: var(--fs-md);
  font-weight: 600;
  cursor: pointer;
}

.link:hover {
  color: var(--color-action-hover);
}

.footer {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
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
