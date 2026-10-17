<script setup lang="ts">
/**
 * Scripts enregistrés : nom, description, confirmation exigée (GO / NO GO)
 * ou exécution directe, dernière exécution, et actions. Pendant une
 * exécution, Exécuter affiche le chrono T+ et ouvre la sortie.
 */
import type { Script } from "../../bindings/Script";
import { formatAgo } from "../../format";
import { useTelemetryStore } from "../../stores/telemetry";
import AppButton from "../ui/AppButton.vue";
import AppIcon from "../ui/AppIcon.vue";
import DataTable from "../ui/DataTable.vue";
import StatusIcon from "../ui/StatusIcon.vue";

defineProps<{
  scripts: Script[];
  /** Horloge de l'ancienneté affichée (« il y a 2 j »). */
  now: number;
}>();

const emit = defineEmits<{
  run: [script: Script];
  edit: [script: Script];
  remove: [script: Script];
}>();

const telemetry = useTelemetryStore();

/** Exécution en cours : début, en horloge `performance.now()` pour le chrono. */
function runningSince(script: Script): number | null {
  const session = telemetry.scriptTab(script.id)?.session;
  if (!session?.active || session.startedAt === null) return null;
  return performance.now() - (Date.now() - session.startedAt);
}

interface LastRunView {
  shape: "disc" | "circle" | "triangle" | "half";
  tone: "success" | "muted" | "danger" | "warning";
  text: string;
  /** Date complète, en infobulle. */
  title: string | undefined;
}

function lastRunView(script: Script, now: number): LastRunView {
  if (runningSince(script) !== null) {
    return {
      shape: "half",
      tone: "warning",
      text: "en cours",
      title: undefined,
    };
  }
  const run = script.lastRun;
  if (run === null) {
    return {
      shape: "circle",
      tone: "muted",
      text: "jamais exécutée",
      title: undefined,
    };
  }
  const ago = formatAgo(run.at, now);
  const title = `Lancée le ${new Date(run.at).toLocaleString("fr-FR")}`;
  if (run.exitCode === null) {
    return {
      shape: "circle",
      tone: "muted",
      text: `${ago} · interrompue`,
      title,
    };
  }
  return run.exitCode === 0
    ? { shape: "disc", tone: "success", text: `${ago} · exit 0`, title }
    : {
        shape: "triangle",
        tone: "danger",
        text: `${ago} · exit ${run.exitCode}`,
        title,
      };
}
</script>

<template>
  <DataTable>
    <colgroup>
      <col class="name" />
      <col class="description" />
      <col class="confirmation" />
      <col class="last-run" />
      <col class="actions" />
    </colgroup>
    <thead>
      <tr>
        <th scope="col">Procédure</th>
        <th scope="col">Description</th>
        <th scope="col">Confirmation</th>
        <th scope="col">Dernière exécution</th>
        <th scope="col" class="actions-head">Actions</th>
      </tr>
    </thead>
    <TransitionGroup name="row-in" tag="tbody" appear>
      <tr v-for="script in scripts" :key="script.id">
        <td class="truncate" :title="script.name">{{ script.name }}</td>
        <td class="description-cell">
          <span class="clamp" :title="script.description || undefined">{{
            script.description || "—"
          }}</span>
        </td>
        <td>
          <span v-if="script.requireConfirmation" class="go-no-go">
            <StatusIcon name="triangle" />
            GO / NO GO
          </span>
          <span v-else class="muted">Directe</span>
        </td>
        <td
          class="truncate last-run-cell"
          :class="lastRunView(script, now).tone"
          :title="lastRunView(script, now).title"
        >
          <StatusIcon class="shape" :name="lastRunView(script, now).shape" />
          {{ lastRunView(script, now).text }}
        </td>
        <td>
          <div class="row-actions">
            <AppButton
              :busy-since="runningSince(script)"
              :aria-label="
                runningSince(script) === null
                  ? `Exécuter ${script.name}`
                  : `Voir la sortie de ${script.name}, en cours`
              "
              @click="emit('run', script)"
            >
              <AppIcon name="play" />
              Exécuter
            </AppButton>
            <AppButton @click="emit('edit', script)">Modifier</AppButton>
            <AppButton variant="danger" @click="emit('remove', script)">
              Supprimer…
            </AppButton>
          </div>
        </td>
      </tr>
    </TransitionGroup>
  </DataTable>
</template>

<style scoped>
/* La description prend la largeur restante. */
.name {
  width: 18%;
}

.confirmation {
  width: 14%;
}

.last-run {
  width: 20%;
}

/* Trois boutons et la place d'un chrono T+. */
.actions {
  width: calc(32ch + 2 * var(--sp-4) + var(--sp-7));
}

.actions-head {
  text-align: right;
}

.description-cell {
  padding-block: var(--sp-2);
  color: var(--color-text-muted);
  font-family: var(--font-ui);
  font-size: var(--fs-sm);
}

/* Deux lignes au plus ; la description complète en infobulle. */
.clamp {
  display: -webkit-box;
  overflow: hidden;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  line-clamp: 2;
}

.go-no-go {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  color: var(--color-warning);
  font-size: var(--fs-sm);
  letter-spacing: var(--tracking-caps);
}

.muted {
  font-size: var(--fs-sm);
}

.last-run-cell {
  color: var(--color-text-muted);
}

.shape {
  margin-right: var(--sp-1);
}

.last-run-cell.success .shape {
  color: var(--color-success);
}

.last-run-cell.warning .shape {
  color: var(--color-warning);
}

.last-run-cell.danger {
  color: var(--color-danger-text);
}

.last-run-cell.danger .shape {
  color: var(--color-danger);
}

.row-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--sp-2);
}
</style>
