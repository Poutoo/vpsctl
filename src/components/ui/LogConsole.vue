<script setup lang="ts">
/**
 * Console de logs : un seul composant, identique partout (panneau ancré du
 * tableau de bord, onglets de l'écran Télémétrie : logs et sortie des
 * scripts). Affiche une session du store `telemetry` : il ne démarre ni
 * n'arrête rien de lui-même, hormis les boutons Interrompre et Rejouer.
 *
 * - Liste virtualisée (@tanstack/vue-virtual), lignes de hauteur fixe
 *   `--console-line-height`, sans retour à la ligne (défilement horizontal).
 * - Défilement automatique désactivable, coupé dès qu'on remonte dans
 *   l'historique ; le bouton « nouvelles lignes » le rétablit.
 * - Légende selon la source : `out` / `err` neutre (Docker, scripts) ou
 *   `inf` / `wrn` / `ERR` rouge (journal systemd, priorité 0 à 3).
 * - Bannière LOS à la fin du flux. Flux coupé par une perte de liaison :
 *   « Reprendre » (logs, à partir de cet instant) ou « Relancer » (script),
 *   jamais automatiquement, et seulement une fois la liaison rétablie.
 */
import { useVirtualizer } from "@tanstack/vue-virtual";
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { formatElapsed, useElapsed } from "../../composables/useElapsed";
import {
  isJournalError,
  type ConsoleLine,
  type LogSession,
} from "../../composables/logSession";
import AppButton from "./AppButton.vue";
import AppIcon from "./AppIcon.vue";
import AppSwitch from "./AppSwitch.vue";

const props = withDefaults(
  defineProps<{
    stream: LogSession;
    /** `output` : Docker et scripts ; `journal` : systemd. */
    legend: "output" | "journal";
    /** Nom accessible de la zone de logs. */
    label: string;
    /** Libellé du bouton qui rouvre le flux terminé (« Relancer » un script). */
    replayLabel?: string;
    /** Sortie d'un script : bannière LOS avec `exit N`, en échec si N ≠ 0. */
    procedure?: boolean;
    /** Remplace l'arrêt direct (ex. confirmation avant d'interrompre un script). */
    requestStop?: () => void;
    /** Remplace la relance directe (ex. confirmation avant de relancer un script). */
    requestReplay?: () => void;
    /** Liaison perdue : relancer ou reprendre est impossible. */
    linkDown?: boolean;
  }>(),
  {
    linkDown: false,
    replayLabel: "Rejouer",
    procedure: false,
    requestStop: undefined,
    requestReplay: undefined,
  },
);

function stop() {
  if (props.requestStop) props.requestStop();
  else void props.stream.stop();
}

function replay() {
  if (props.requestReplay) props.requestReplay();
  else void props.stream.replay();
}

/** Logs coupés par un LOS : reprise à partir de maintenant, sans rattrapage. */
const resumable = computed(
  () => props.stream.status === "lost" && !props.procedure,
);

defineSlots<{
  /** Identification de la source, à gauche de l'en-tête. */
  title(): unknown;
  /** Boutons supplémentaires, à droite (ex. fermer le panneau). */
  actions?(): unknown;
}>();

const scroller = ref<HTMLElement | null>(null);
/** Hauteur d'une ligne en px, lue dans le token `--console-line-height`. */
const lineHeight = ref(18);

onMounted(() => {
  const token = getComputedStyle(document.documentElement).getPropertyValue(
    "--console-line-height",
  );
  const parsed = Number.parseFloat(token);
  if (Number.isFinite(parsed) && parsed > 0) lineHeight.value = parsed;
  // Flux déjà alimenté (retour sur un onglet, « Plein écran ») : on reprend
  // à la fin, défilement automatique actif.
  if (props.stream.lines.length > 0) scrollToEnd();
});

const virtualizer = useVirtualizer(
  computed(() => ({
    count: props.stream.lines.length,
    getScrollElement: () => scroller.value,
    estimateSize: () => lineHeight.value,
    overscan: 20,
  })),
);
const rows = computed(() => virtualizer.value.getVirtualItems());
const totalHeight = computed(() => virtualizer.value.getTotalSize());

// --- Défilement automatique -----------------------------------------------

const follow = ref(true);
/** Lignes arrivées pendant que le défilement automatique est coupé. */
const unseen = ref(0);

function scrollToEnd() {
  void nextTick(() => {
    const element = scroller.value;
    if (element) element.scrollTop = element.scrollHeight;
  });
}

watch(
  () => props.stream.received,
  (received, previous) => {
    if (received < previous) {
      unseen.value = 0;
      return;
    }
    if (follow.value) scrollToEnd();
    else unseen.value += received - previous;
  },
);

watch(follow, (on) => {
  if (on) {
    unseen.value = 0;
    scrollToEnd();
  }
});

/** Remonter dans l'historique coupe le défilement automatique. */
function onScroll() {
  const element = scroller.value;
  if (!element || !follow.value) return;
  const fromEnd =
    element.scrollHeight - element.scrollTop - element.clientHeight;
  if (fromEnd > lineHeight.value) follow.value = false;
}

function jumpToEnd() {
  follow.value = true;
  unseen.value = 0;
  scrollToEnd();
}

// --- Lignes -----------------------------------------------------------------

/** Heure locale `18:19:05.313`. */
function clock(iso: string | null): string {
  if (iso === null) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  const pad = (n: number, width = 2) => String(n).padStart(width, "0");
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${pad(date.getMilliseconds(), 3)}`;
}

/** Étiquette d'origine : `out` / `err`, ou `inf` / `wrn` / `ERR`. */
function tag(line: ConsoleLine): string {
  if (line.notice) return "";
  if (line.origin === "journal") {
    const priority = line.priority ?? 6;
    if (priority <= 3) return "ERR";
    return priority === 4 ? "wrn" : "inf";
  }
  return line.origin === "stderr" ? "err" : "out";
}

function lineClass(line: ConsoleLine) {
  return {
    error: isJournalError(line),
    warning: line.origin === "journal" && line.priority === 4,
    stderr: line.origin === "stderr",
    discreet: line.missing || line.notice,
  };
}

// --- États -------------------------------------------------------------------

const startingElapsed = useElapsed(computed(() => props.stream.startingSince));

/** Script coupé par un LOS : son issue sur le serveur est inconnue. */
const SCRIPT_LOST_MESSAGE =
  "Le script a été interrompu sur le serveur, probablement en cours d'exécution. Vérifiez l'état du serveur avant de le relancer.";

const los = computed(() => {
  const { status, exitCode, error, endedAt } = props.stream;
  if (
    status !== "ended" &&
    status !== "stopped" &&
    status !== "error" &&
    status !== "lost"
  ) {
    return null;
  }
  const at =
    endedAt === null
      ? null
      : new Date(endedAt).toLocaleTimeString("fr-FR", { hour12: false });
  if (status === "lost") {
    const what = props.procedure ? "Script interrompu" : "Flux interrompu";
    return props.linkDown
      ? {
          detail: `Liaison perdue · ${props.procedure ? "issue inconnue" : "flux interrompu"}`,
          at,
          failed: true,
          message: props.procedure ? SCRIPT_LOST_MESSAGE : null,
          signal: true,
        }
      : {
          detail: props.procedure
            ? `${what} à ${at} · pas de code de sortie`
            : `${what} à ${at} · Reprendre le relance à partir de maintenant`,
          at: null,
          failed: false,
          message: props.procedure ? SCRIPT_LOST_MESSAGE : null,
          signal: false,
        };
  }
  if (props.procedure) {
    const detail =
      status === "stopped"
        ? "Procédure interrompue"
        : status === "error"
          ? (error ?? "Flux interrompu")
          : exitCode === null
            ? "Fin du flux · tuée par un signal"
            : `Fin du flux · exit ${exitCode}`;
    return {
      detail,
      at,
      failed: status === "error" || (status === "ended" && exitCode !== 0),
      message: null,
      signal: true,
    };
  }
  const detail =
    status === "stopped"
      ? "Arrêt propre"
      : status === "error"
        ? (error ?? "Flux interrompu")
        : exitCode === null
          ? "Fin du flux"
          : `Fin du flux · code ${exitCode}`;
  return {
    detail,
    at,
    failed: status === "error",
    message: null,
    signal: true,
  };
});

const counts = computed(() => {
  const total = props.stream.lines.length;
  const lines = `${total} ${total > 1 ? "lignes" : "ligne"}`;
  if (props.legend === "journal") {
    return `${lines} · ${props.stream.errorCount} ERR`;
  }
  return `${lines} · ${props.stream.stderrCount} sur stderr`;
});
</script>

<template>
  <section class="console" :aria-label="label">
    <header class="head">
      <div class="ident"><slot name="title" /></div>
      <div class="controls">
        <AppSwitch v-model="follow" label="Défilement auto" />
        <AppButton size="compact" variant="quiet" @click="stream.clear()">
          Effacer
        </AppButton>
        <AppButton v-if="stream.active" size="compact" @click="stop">
          Interrompre le flux
        </AppButton>
        <AppButton
          v-else-if="resumable"
          size="compact"
          :variant="linkDown ? 'secondary' : 'primary'"
          :disabled="linkDown"
          @click="stream.resume()"
        >
          Reprendre
        </AppButton>
        <AppButton v-else size="compact" :disabled="linkDown" @click="replay">
          {{ replayLabel }}
        </AppButton>
        <slot name="actions" />
      </div>
    </header>

    <p
      v-if="stream.status === 'starting' && startingElapsed !== null"
      class="caps strip"
      role="status"
    >
      Acquisition du flux · {{ formatElapsed(startingElapsed, "ms") }}
    </p>

    <div class="body">
      <div
        ref="scroller"
        class="scroller"
        role="log"
        aria-live="off"
        tabindex="0"
        @scroll.passive="onScroll"
      >
        <p
          v-if="stream.status === 'live' && stream.lines.length === 0"
          class="waiting"
        >
          Aucune ligne pour l'instant.
        </p>
        <div class="sizer" :style="{ height: `${totalHeight}px` }">
          <div
            v-for="row in rows"
            :key="stream.lines[row.index].id"
            class="line"
            :class="lineClass(stream.lines[row.index])"
            :style="{ transform: `translateY(${row.start}px)` }"
          >
            <span class="time">{{
              clock(stream.lines[row.index].timestamp)
            }}</span>
            <span class="origin">{{ tag(stream.lines[row.index]) }}</span>
            <span class="text">{{ stream.lines[row.index].text }}</span>
          </div>
        </div>
        <Transition name="los" appear>
          <p v-if="los" class="los" :class="{ failed: los.failed }">
            <span class="rule" aria-hidden="true" />
            <span class="caps los-text">
              <AppIcon name="signal-lost" />
              <template v-if="los.signal">LOS · </template>{{ los.detail }}
              <template v-if="los.at"> · {{ los.at }}</template>
            </span>
            <span class="rule" aria-hidden="true" />
          </p>
        </Transition>
        <p v-if="los?.message" class="los-message">{{ los.message }}</p>
      </div>

      <AppButton
        v-if="unseen > 0"
        class="new-lines"
        variant="primary"
        @click="jumpToEnd"
      >
        <AppIcon name="arrow-down" />
        {{ unseen }} {{ unseen > 1 ? "nouvelles lignes" : "nouvelle ligne" }}
      </AppButton>
    </div>

    <footer class="foot">
      <p v-if="legend === 'journal'" class="legend">
        <span><span class="key">inf</span> information</span>
        <span><span class="key">wrn</span> avertissement · priorité 4</span>
        <span><span class="key key-error">ERR</span> priorité 0 à 3</span>
      </p>
      <p v-else class="legend">
        <span><span class="key">out</span> sortie standard</span>
        <span
          ><span class="key">err</span> sortie d'erreur, sans gravité
          implicite</span
        >
      </p>
      <span class="counts">{{ counts }}</span>
    </footer>
  </section>
</template>

<style scoped>
.console {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

/*
 * Une seule ligne : les boutons gardent leur taille, l'identification de la
 * source prend la place restante et coupe sa commande (« … »).
 */
.head {
  display: flex;
  flex-wrap: nowrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  padding: var(--sp-2) var(--sp-3) var(--sp-2) var(--sp-4);
  background: var(--color-surface-raised);
  border-bottom: var(--border-width) solid var(--color-border);
}

.ident {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--sp-3);
  min-width: 0;
}

/* Boutons compacts : l'en-tête reste une barre fine au-dessus des logs. */
.controls {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: var(--sp-2);
}

.strip {
  padding: var(--sp-1) var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
  color: var(--color-warning);
}

.body {
  position: relative;
  flex: 1;
  min-height: 0;
}

/* Surface opaque sous les logs, jamais l'image de fond. */
.scroller {
  height: 100%;
  overflow: auto;
  padding: var(--sp-2) 0;
  background: var(--color-field);
  font-family: var(--font-mono);
  font-size: var(--fs-data);
  line-height: var(--console-line-height);
}

.waiting {
  padding: 0 var(--sp-4);
  color: var(--color-text-subtle);
}

.sizer {
  position: relative;
}

.line {
  position: absolute;
  top: 0;
  left: 0;
  display: flex;
  gap: var(--sp-4);
  min-width: 100%;
  height: var(--console-line-height);
  padding: 0 var(--sp-4);
  white-space: pre;
}

.time {
  flex-shrink: 0;
  width: 12ch;
  color: var(--color-text-subtle);
}

.origin {
  flex-shrink: 0;
  width: 3ch;
  color: var(--color-text-muted);
}

.text {
  color: var(--color-text);
}

/* Sortie d'erreur : neutre (beaucoup de programmes y écrivent tous leurs logs). */
.stderr .origin {
  color: var(--color-text-subtle);
}

.warning .origin {
  color: var(--color-warning);
}

/* Erreur du journal (priorité 0 à 3) : étiquette, texte et fond rouges. */
.error {
  background: var(--color-danger-tint);
}

.error .origin,
.error .text {
  color: var(--color-danger-text);
}

/* Entrée sans message, avis de lignes écartées. */
.discreet .text {
  color: var(--color-text-subtle);
}

.los {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  margin: var(--sp-2) var(--sp-4) 0;
  color: var(--color-text-muted);
}

.los.failed {
  color: var(--color-danger-text);
}

.rule {
  flex: 1;
  border-top: var(--border-width) solid var(--color-border-strong);
}

.failed .rule {
  border-top-color: var(--color-danger);
}

.los-text {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  white-space: nowrap;
}

.los-message {
  max-width: 72ch;
  margin: var(--sp-3) auto 0;
  padding: 0 var(--sp-4);
  color: var(--color-text);
  font-family: var(--font-ui);
  font-size: var(--fs-sm);
  text-align: center;
}

.los-enter-active {
  transition: opacity var(--dur-fast) var(--ease-out);
}

.los-enter-from {
  opacity: 0;
}

.new-lines {
  position: absolute;
  right: var(--sp-4);
  bottom: var(--sp-3);
}

.foot {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  padding: var(--sp-2) var(--sp-4);
  background: var(--color-surface-raised);
  border-top: var(--border-width) solid var(--color-border);
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.legend {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-4);
}

.key {
  margin-right: var(--sp-1);
  color: var(--color-text);
  font-family: var(--font-mono);
}

.key-error {
  color: var(--color-danger-text);
}

.counts {
  font-family: var(--font-mono);
}
</style>
