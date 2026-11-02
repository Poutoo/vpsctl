<script setup lang="ts">
/**
 * En-tête d'un projet compose dans le tableau des conteneurs : bouton de
 * repli, nom, dossier, état agrégé, détail et actions du projet ; puis, au
 * besoin, une ligne d'explication (dossier ou fichier en défaut, projet
 * descendu). Pendant une commande, le verbe et son chrono remplacent l'état ;
 * après un échec, la colonne Détail mène à la sortie jusqu'à la commande
 * suivante.
 */
import { computed } from "vue";
import type { ComposeAction } from "../../bindings/ComposeAction";
import type { ComposeProject } from "../../bindings/ComposeProject";
import type { FilesStatus } from "../../bindings/FilesStatus";
import {
  DOWN_ACTION,
  PROJECT_ACTIONS,
  projectActionReasons,
} from "../../composeActions";
import {
  baseName,
  filesProblem,
  projectCount,
  projectDetail,
  projectDetailHelp,
} from "../../composeView";
import { useComposeStore } from "../../stores/compose";
import { useConnectionStore } from "../../stores/connection";
import AppIcon from "../ui/AppIcon.vue";
import HighlightText from "../ui/HighlightText.vue";
import IconButton from "../ui/IconButton.vue";
import ProjectStateLabel from "../ui/ProjectStateLabel.vue";
import StatusIcon from "../ui/StatusIcon.vue";

const props = defineProps<{
  project: ComposeProject;
  /** Conteneurs affichés sous l'en-tête (après le filtre). */
  shown: number;
  expanded: boolean;
  query: string;
  /** Dernière vérification du dossier et des fichiers, si faite. */
  check: FilesStatus | null;
  /** Plugin compose présent (ou non détecté) : actions de projet affichées. */
  actions: boolean;
}>();

const emit = defineEmits<{
  toggle: [];
  forget: [];
  run: [action: ComposeAction];
  showTab: [tabId: string];
}>();

const connection = useConnectionStore();
const compose = useComposeStore();

const down = computed(() => props.project.state === "down");
const workingDir = computed(() => props.project.files?.workingDir ?? null);
const problem = computed(() =>
  filesProblem(props.check, workingDir.value ?? ""),
);
const running = computed(() => compose.running(props.project.name));
const outcome = computed(() =>
  running.value ? null : compose.outcome(props.project.name),
);
const failed = computed(
  () =>
    !running.value &&
    (props.project.state === "failed" || problem.value !== null),
);
const detail = computed(() => {
  if (running.value) return `docker compose ${running.value.action} · en cours`;
  if (problem.value) return problem.value.short;
  return projectDetail(props.project);
});
const reasons = computed(() =>
  projectActionReasons(props.project, props.check, {
    lost: connection.lost,
    busy: running.value !== null,
  }),
);
const files = computed(() =>
  (props.project.files?.configFiles ?? []).map(baseName).join(" + "),
);
const containerCount = computed(() => {
  const count = props.project.state === "down" ? 0 : props.shown;
  return `${count} ${count > 1 ? "conteneurs" : "conteneur"}`;
});

function busySince(action: ComposeAction): number | null {
  return running.value?.action === action ? running.value.since : null;
}
</script>

<template>
  <tr
    class="project-row"
    :class="{ failed }"
    :data-testid="`project-${project.name}`"
  >
    <td>
      <div class="name-cell">
        <button
          type="button"
          class="toggle"
          :aria-expanded="expanded"
          :aria-label="`${expanded ? 'Replier' : 'Déplier'} le projet ${project.name}`"
          @click="emit('toggle')"
        >
          <AppIcon :name="expanded ? 'chevron-down' : 'chevron-right'" />
        </button>
        <span class="title truncate" :title="project.name">
          <HighlightText :text="project.name" :query="query" />
        </span>
        <span class="caps meta">{{ containerCount }}</span>
      </div>
    </td>
    <td
      class="truncate dir-cell"
      data-testid="project-dir"
      :class="{ missing: problem?.dirMissing }"
      :title="workingDir ?? undefined"
    >
      <template v-if="workingDir">
        <AppIcon
          class="folder"
          :name="problem?.dirMissing ? 'folder-missing' : 'folder'"
        />
        <span class="dir-path"
          ><HighlightText :text="workingDir" :query="query"
        /></span>
      </template>
      <template v-else>—</template>
    </td>
    <td class="state-cell" data-testid="project-state">
      <ProjectStateLabel
        :state="project.state"
        :count="projectCount(project)"
        :label="running?.action ?? null"
        :since="running?.since ?? null"
      />
      <span
        class="stacked-detail"
        :class="{
          problem: !running && (outcome || project.problem || problem),
          busy: running,
        }"
        :title="projectDetailHelp(project)"
      >
        <template v-if="outcome">
          {{ outcome.action }} · {{ outcome.text }}
        </template>
        <template v-else>
          <StatusIcon
            v-if="problem && !running"
            class="problem-shape"
            name="triangle"
          />
          {{ detail }}
        </template>
      </span>
    </td>
    <td
      class="truncate detail-cell"
      data-testid="project-detail"
      :class="{
        problem: !running && (outcome || project.problem || problem),
        busy: running,
      }"
      :title="projectDetailHelp(project)"
    >
      <button
        v-if="outcome && outcome.tabId"
        type="button"
        class="outcome"
        @click="emit('showTab', outcome.tabId)"
      >
        {{ outcome.action }} · {{ outcome.text }} · voir
        <AppIcon name="arrow-right" />
      </button>
      <template v-else-if="outcome">
        {{ outcome.action }} · {{ outcome.text }}
      </template>
      <template v-else>
        <StatusIcon
          v-if="problem && !running"
          class="problem-shape"
          name="triangle"
        />
        {{ detail }}
      </template>
    </td>
    <td>
      <div class="row-actions">
        <template v-if="actions">
          <IconButton
            v-for="meta in PROJECT_ACTIONS"
            :key="meta.action"
            :icon="meta.icon"
            :label="`${meta.label} le projet ${project.name}`"
            :hint="meta.hint"
            :unavailable="reasons[meta.action]"
            :busy-since="busySince(meta.action)"
            @click="emit('run', meta.action)"
          />
        </template>
        <template v-if="down">
          <span class="separator" aria-hidden="true" />
          <IconButton
            icon="forget"
            :label="`Oublier le projet ${project.name}`"
            hint="Le retire de la liste de ce profil, sans rien toucher sur le serveur."
            :unavailable="connection.lost ? 'liaison perdue' : null"
            @click="emit('forget')"
          />
        </template>
        <template v-else-if="actions">
          <span class="separator" aria-hidden="true" />
          <IconButton
            :icon="DOWN_ACTION.icon"
            tone="danger"
            :label="`${DOWN_ACTION.label} le projet ${project.name}`"
            :hint="DOWN_ACTION.hint"
            :unavailable="reasons.down"
            :busy-since="busySince('down')"
            @click="emit('run', 'down')"
          />
        </template>
      </div>
    </td>
  </tr>
  <tr
    v-if="problem"
    class="note-row"
    :data-testid="`project-problem-${project.name}`"
  >
    <td colspan="5">
      <p class="note">
        <StatusIcon class="note-shape danger" name="triangle" />
        <span>
          <strong>{{ problem.text }}</strong>
          <template v-if="project.running > 0">
            Les conteneurs tournent toujours.</template
          >
          <template v-if="actions">
            Démarrer et Récupérer les images ont besoin de
            {{ files || "ses fichiers" }} ; Redémarrer et Arrêter et supprimer
            passent par le nom du projet et restent possibles.</template
          >
        </span>
      </p>
    </td>
  </tr>
  <tr
    v-if="down"
    class="note-row"
    :data-testid="`project-down-${project.name}`"
  >
    <td colspan="5">
      <p class="note">
        <StatusIcon class="note-shape" name="circle" />
        <span>
          <strong>Aucun conteneur.</strong>
          Docker ne connaît plus ce projet. Atlas l'a mémorisé pour ce profil
          avec son dossier {{ workingDir
          }}<template v-if="actions"
            >, et Démarrer le recrée depuis ce dossier</template
          >. Oublier ce projet le retire de la liste, sans toucher au serveur.
        </span>
      </p>
    </td>
  </tr>
</template>

<style scoped>
.name-cell {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  min-width: 0;
}

.toggle {
  display: inline-flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  padding: 0;
  background: none;
  border: none;
  color: var(--color-text-muted);
  cursor: pointer;
}

.toggle:hover {
  color: var(--color-text);
}

.title {
  min-width: 0;
  font-family: var(--font-title);
  font-size: var(--fs-lg);
  font-weight: 600;
}

.meta {
  flex-shrink: 0;
  color: var(--color-text-muted);
  font-size: var(--fs-xs);
  font-weight: 400;
}

.dir-cell {
  color: var(--color-text-muted);
}

/* Dossier introuvable : chemin barré, contour pointillé, texte d'erreur. */
.dir-cell.missing {
  color: var(--color-danger-text);
}

.missing .dir-path {
  text-decoration: line-through;
}

.problem-shape {
  color: var(--color-danger);
}

.folder {
  margin-right: var(--sp-2);
}

.detail-cell,
.stacked-detail {
  color: var(--color-text-muted);
}

.problem {
  color: var(--color-danger-text);
}

.busy {
  color: var(--color-warning);
}

/* Dernier échec : lien vers la sortie, texte d'erreur. */
.outcome {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-1);
  max-width: 100%;
  padding: 0;
  overflow: hidden;
  background: none;
  border: none;
  color: var(--color-danger-text);
  font: inherit;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}

.outcome:hover {
  text-decoration: underline;
}

.stacked-detail {
  display: none;
}

@media (max-width: 1100px) {
  .state-cell {
    padding-top: var(--sp-1);
    padding-bottom: var(--sp-1);
  }

  .stacked-detail {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .detail-cell {
    padding: 0;
    font-size: 0;
  }
}

.row-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--sp-1);
}

.separator {
  align-self: stretch;
  margin: var(--sp-1) var(--sp-1);
  border-left: var(--border-width) solid var(--color-border-strong);
}

/* Ligne d'explication sous l'en-tête : texte courant, pas de donnée. */
.note-row td {
  height: auto;
  padding: var(--sp-2) var(--sp-4) var(--sp-2) var(--sp-7);
}

.note {
  display: flex;
  gap: var(--sp-3);
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--fs-sm);
  font-weight: 400;
}

.note-shape {
  flex-shrink: 0;
  margin-top: var(--sp-1);
  color: var(--color-text-muted);
}

.note-shape.danger {
  color: var(--color-danger);
}
</style>
