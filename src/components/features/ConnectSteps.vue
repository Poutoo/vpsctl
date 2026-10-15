<script setup lang="ts">
/**
 * Télémétrie de liaison : étapes réelles de la connexion, horodatées depuis
 * le début de la tentative, et bannière LOS en cas d'échec.
 */
import { computed, ref, watch } from "vue";
import type { ConnectStep } from "../../bindings/ConnectStep";
import { formatElapsed, useElapsed } from "../../composables/useElapsed";
import type { ConnectPhase, TimedStep } from "../../stores/connection";
import AppIcon from "../ui/AppIcon.vue";
import StatusIcon from "../ui/StatusIcon.vue";

const props = defineProps<{
  steps: TimedStep[];
  phase: ConnectPhase;
  /** `hôte:port` visé, pour l'étape TCP. */
  endpoint: string;
  profileName: string | null;
  /** Début de la tentative en cours (`performance.now()`), pour le chrono. */
  startedAt: number | null;
}>();

type Mark = "ok" | "pending" | "error";

interface Row {
  at: string;
  label: string;
  mark: Mark;
  text: string;
  detail: string | null;
}

/** Libellé accessible de chaque marque (la forme ne suffit pas seule). */
const MARK_LABEL: Record<Mark, string> = {
  ok: "réussi",
  pending: "en attente",
  error: "échec",
};

function seconds(ms: number): string {
  return formatElapsed(ms, "ms");
}

function describe(step: ConnectStep): Omit<Row, "at"> {
  switch (step.step) {
    case "tcp":
      return step.status === "ok"
        ? {
            label: "TCP",
            mark: "ok",
            text: `connexion à ${props.endpoint}`,
            detail: `établie · ${step.durationMs ?? "?"} ms`,
          }
        : {
            label: "TCP",
            mark: "error",
            text: `connexion à ${props.endpoint}`,
            detail: step.message,
          };
    case "ssh":
      return step.status === "ok"
        ? {
            label: "SSH",
            mark: "ok",
            text: "négociation",
            detail: [step.kex, step.cipher].filter(Boolean).join(" · ") || null,
          }
        : {
            label: "SSH",
            mark: "error",
            text: "négociation",
            detail: step.message,
          };
    case "hostKey":
      return {
        label: "HÔTE",
        mark:
          step.status === "ok"
            ? "ok"
            : step.status === "unknown"
              ? "pending"
              : "error",
        text:
          step.status === "ok"
            ? "clé vérifiée"
            : step.status === "unknown"
              ? "clé inconnue · comparaison requise"
              : "empreinte différente de celle mémorisée",
        detail: step.fingerprint,
      };
    case "auth":
      return {
        label: "AUTH",
        mark:
          step.status === "ok"
            ? "ok"
            : step.status === "passphraseRequired"
              ? "pending"
              : "error",
        text:
          step.status === "ok"
            ? "authentifié par clé"
            : step.status === "passphraseRequired"
              ? "passphrase requise"
              : "échec",
        detail: step.message,
      };
    case "aos":
      return {
        label: "AOS",
        mark: "ok",
        text: "liaison établie",
        detail: null,
      };
  }
}

const rows = computed<Row[]>(() =>
  props.steps.map(({ step, atMs }) => ({
    at: seconds(atMs),
    ...describe(step),
  })),
);

const closedBeforeAuth = computed(
  () => props.phase === "hostKeyUnknown" || props.phase === "hostKeyMismatch",
);

const connecting = computed(() => props.phase === "connecting");
const live = useElapsed(
  computed(() => (connecting.value ? props.startedAt : null)),
);

/** Durée de la dernière tentative, figée quand elle se termine. */
const endedAfter = ref<number | null>(null);
watch(
  () => props.phase,
  (phase, previous) => {
    if (phase === "connecting") endedAfter.value = null;
    else if (previous === "connecting" && props.startedAt !== null) {
      endedAfter.value = performance.now() - props.startedAt;
    }
  },
);

/** Chrono de l'en-tête : en direct pendant la connexion, figé ensuite. */
const chrono = computed(() => {
  if (live.value !== null) return seconds(live.value);
  if (endedAfter.value !== null) return seconds(endedAfter.value);
  const last = props.steps[props.steps.length - 1];
  return seconds(last?.atMs ?? 0);
});
</script>

<template>
  <section class="console" aria-labelledby="telemetry-title">
    <header class="header">
      <h2 id="telemetry-title" class="caps title">Télémétrie de liaison</h2>
      <span v-if="profileName" class="profile">{{ profileName }}</span>
      <span class="command">ssh -v</span>
      <span class="chrono">{{ chrono }}</span>
    </header>

    <div class="output" aria-live="polite">
      <p v-if="rows.length === 0 && !connecting" class="empty">
        — aucune séquence en cours —
      </p>
      <ol v-else class="rows">
        <li v-for="(row, index) in rows" :key="index" :class="row.mark">
          <span class="at">{{ row.at }}</span>
          <span class="mark" role="img" :aria-label="MARK_LABEL[row.mark]">
            <StatusIcon v-if="row.mark === 'ok'" name="check" />
            <StatusIcon v-else-if="row.mark === 'error'" name="cross" />
            <template v-else>?</template>
          </span>
          <span class="label">{{ row.label }}</span>
          <span class="text">{{ row.text }}</span>
          <span v-if="row.detail" class="detail" :title="row.detail">{{
            row.detail
          }}</span>
        </li>
        <li v-if="connecting" class="pending">
          <span class="at" />
          <span class="mark" aria-hidden="true">…</span>
          <span class="label" />
          <span class="text">en cours</span>
        </li>
        <li v-if="closedBeforeAuth" class="closed">
          <span class="at" />
          <span class="mark" role="img" aria-label="fermée">
            <StatusIcon name="circle" />
          </span>
          <span class="label">LIAISON</span>
          <span class="text">fermée avant authentification</span>
          <span class="detail">aucune donnée envoyée</span>
        </li>
      </ol>

      <Transition name="los">
        <p v-if="phase === 'failed'" class="los" role="status">
          <AppIcon name="signal-lost" />
          <span class="caps">LOS · échec de liaison · {{ chrono }}</span>
        </p>
      </Transition>
    </div>
  </section>
</template>

<style scoped>
.console {
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border);
  font-family: var(--font-mono);
  font-size: var(--fs-data);
}

.header {
  display: flex;
  align-items: baseline;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-4);
  background: var(--color-surface-raised);
  border-bottom: var(--border-width) solid var(--color-border);
}

.title {
  color: var(--color-text-muted);
}

.profile {
  font-weight: 600;
}

.command {
  color: var(--color-text-muted);
}

.chrono {
  margin-left: auto;
  color: var(--color-text-muted);
}

.output {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--sp-3) var(--sp-4);
}

.empty {
  color: var(--color-text-muted);
}

.rows {
  margin: 0;
  padding: 0;
  list-style: none;
}

.rows li {
  display: grid;
  grid-template-columns: 9ch 2ch 8ch minmax(0, 1fr) auto;
  align-items: baseline;
  column-gap: var(--sp-3);
  padding: var(--sp-1) var(--sp-2);
}

.at {
  color: var(--color-text-muted);
}

.text {
  overflow: hidden;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.detail {
  max-width: 32ch;
  overflow: hidden;
  color: var(--color-text-muted);
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ok .mark {
  color: var(--color-success);
}

.pending .mark {
  color: var(--color-warning);
}

.closed .mark {
  color: var(--color-text-muted);
}

/* Le symbole est une forme : --color-danger ; le texte : --color-danger-text. */
.error {
  background: var(--color-danger-tint);
}

.error .mark {
  color: var(--color-danger);
}

.error .text,
.error .detail {
  color: var(--color-danger-text);
}

/* Bannière LOS : filets de part et d'autre, fondu en --dur-fast. */
.los {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  margin-top: var(--sp-3);
  color: var(--color-danger-text);
}

.los::before,
.los::after {
  content: "";
  flex: 1;
  border-top: var(--border-width) solid var(--color-danger);
}

.los :deep(.icon) {
  color: var(--color-danger);
}

.los-enter-active {
  transition: opacity var(--dur-fast) var(--ease-out);
}

.los-enter-from {
  opacity: 0;
}
</style>
