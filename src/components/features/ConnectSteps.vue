<script setup lang="ts">
import { computed } from "vue";
import type { ConnectStep } from "../../bindings/ConnectStep";
import type { ConnectPhase, TimedStep } from "../../stores/connection";

const props = defineProps<{
  steps: TimedStep[];
  phase: ConnectPhase;
  /** `hôte:port` visé, pour l'étape TCP. */
  endpoint: string;
}>();

type Mark = "ok" | "pending" | "error";

interface Row {
  at: string;
  label: string;
  mark: Mark;
  text: string;
  detail: string | null;
}

const MARK_SYMBOL: Record<Mark, string> = { ok: "✓", pending: "?", error: "✗" };

function seconds(ms: number): string {
  return `T+${(ms / 1000).toFixed(3)}`;
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
</script>

<template>
  <section class="telemetry" aria-live="polite">
    <p v-if="rows.length === 0 && phase !== 'connecting'" class="empty">
      Aucune séquence en cours.
    </p>
    <ol v-else class="rows">
      <li v-for="(row, index) in rows" :key="index" :class="row.mark">
        <span class="at">{{ row.at }}</span>
        <span class="mark" :aria-label="row.mark">{{
          MARK_SYMBOL[row.mark]
        }}</span>
        <span class="label">{{ row.label }}</span>
        <span class="text">{{ row.text }}</span>
        <span v-if="row.detail" class="detail">{{ row.detail }}</span>
      </li>
      <li v-if="phase === 'connecting'" class="pending">
        <span class="at" />
        <span class="mark">…</span>
        <span class="text">en cours</span>
      </li>
      <li v-if="closedBeforeAuth" class="pending">
        <span class="at" />
        <span class="mark">○</span>
        <span class="label">LIAISON</span>
        <span class="text">fermée avant authentification</span>
        <span class="detail">aucune donnée d'authentification envoyée</span>
      </li>
    </ol>
  </section>
</template>

<style scoped>
.telemetry {
  font-family: var(--font-mono);
  font-size: var(--font-size-sm);
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
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
  padding: var(--space-1) 0;
}

.at {
  min-width: 9ch;
  color: var(--color-text-muted);
}

.label {
  min-width: 7ch;
}

.detail {
  margin-left: auto;
  color: var(--color-text-muted);
  word-break: break-all;
}

.ok .mark {
  color: var(--color-success);
}

.pending .mark {
  color: var(--color-warning);
}

.error .mark,
.error .detail {
  color: var(--color-danger);
}
</style>
