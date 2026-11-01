<script setup lang="ts">
/**
 * En-tête d'un projet compose dans le tableau des conteneurs : bouton de
 * repli, nom, dossier, état agrégé, détail et actions du projet ; puis, au
 * besoin, une ligne d'explication (projet descendu).
 */
import { computed } from "vue";
import type { ComposeProject } from "../../bindings/ComposeProject";
import {
  projectCount,
  projectDetail,
  projectDetailHelp,
} from "../../composeView";
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
}>();

const emit = defineEmits<{ toggle: []; forget: [] }>();

const connection = useConnectionStore();

const down = computed(() => props.project.state === "down");
const workingDir = computed(() => props.project.files?.workingDir ?? null);
const containerCount = computed(() => {
  const count = props.project.state === "down" ? 0 : props.shown;
  return `${count} ${count > 1 ? "conteneurs" : "conteneur"}`;
});
</script>

<template>
  <tr class="project-row" :class="{ failed: project.state === 'failed' }">
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
    <td class="truncate dir-cell" :title="workingDir ?? undefined">
      <template v-if="workingDir">
        <AppIcon class="folder" name="folder" />
        <HighlightText :text="workingDir" :query="query" />
      </template>
      <template v-else>—</template>
    </td>
    <td class="state-cell">
      <ProjectStateLabel
        :state="project.state"
        :count="projectCount(project)"
      />
      <span
        class="stacked-detail"
        :class="{ problem: project.problem }"
        :title="projectDetailHelp(project)"
        >{{ projectDetail(project) }}</span
      >
    </td>
    <td
      class="truncate detail-cell"
      :class="{ problem: project.problem }"
      :title="projectDetailHelp(project)"
    >
      {{ projectDetail(project) }}
    </td>
    <td>
      <div class="row-actions">
        <template v-if="down">
          <span class="separator" aria-hidden="true" />
          <IconButton
            icon="forget"
            :label="`Oublier le projet ${project.name}`"
            :unavailable="connection.lost ? 'liaison perdue' : null"
            @click="emit('forget')"
          />
        </template>
      </div>
    </td>
  </tr>
  <tr v-if="down" class="note-row">
    <td colspan="5">
      <p class="note">
        <StatusIcon class="note-shape" name="circle" />
        <span>
          <strong>Aucun conteneur.</strong>
          Docker ne connaît plus ce projet. Atlas l'a mémorisé pour ce profil,
          avec son dossier {{ workingDir }}. Oublier ce projet le retire de la
          liste, sans toucher au serveur.
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
</style>
