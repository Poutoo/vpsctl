<script setup lang="ts">
/**
 * Conteneurs Docker groupés par projet compose. L'en-tête d'un projet porte
 * son dossier, son état agrégé et ses actions ; ses conteneurs gardent les
 * leurs. Les conteneurs sans étiquette de projet forment un dernier groupe,
 * « Hors compose », jamais replié.
 */
import { computed } from "vue";
import type { ComposeAction } from "../../bindings/ComposeAction";
import type { ComposeProject } from "../../bindings/ComposeProject";
import type { Container } from "../../bindings/Container";
import { type ContainerGroup } from "../../composeView";
import { useComposeStore } from "../../stores/compose";
import { useServicesStore, type Operation } from "../../stores/services";
import DataTable from "../ui/DataTable.vue";
import FilterNoMatch from "../ui/FilterNoMatch.vue";
import ComposeProjectRows from "./ComposeProjectRows.vue";
import ContainerRow from "./ContainerRow.vue";

const props = withDefaults(
  defineProps<{
    /** Groupes affichés, déjà filtrés. */
    groups: ContainerGroup[];
    /** Premier chargement : registre tracé et vide. */
    loading?: boolean;
    /** Filtre en cours, surligné dans les noms et les images. */
    query?: string;
    /** Conteneurs de la liste complète, avant le filtre. */
    total?: number;
    /** Plugin compose présent (ou non détecté) : actions de projet affichées. */
    composeActions?: boolean;
  }>(),
  { loading: false, query: "", total: 0, composeActions: true },
);

const emit = defineEmits<{
  logs: [container: Container];
  operate: [container: Container, operation: Operation];
  forget: [project: ComposeProject];
  run: [project: ComposeProject, action: ComposeAction];
  showTab: [tabId: string];
  clearFilter: [];
}>();

const store = useServicesStore();
const compose = useComposeStore();

/** Lignes vides tracées pendant le premier chargement. */
const PLACEHOLDER_ROWS = 3;

function groupKey(group: ContainerGroup): string {
  return group.kind === "project" ? `project:${group.project.name}` : "loose";
}

function expanded(group: ContainerGroup): boolean {
  return group.kind === "loose" || !store.collapsed[group.project.name];
}

function branch(group: ContainerGroup, index: number): "middle" | "last" {
  return index === group.containers.length - 1 ? "last" : "middle";
}

const looseCount = computed(() => {
  const loose = props.groups.find((g) => g.kind === "loose");
  return loose ? loose.containers.length : 0;
});
</script>

<template>
  <DataTable>
    <colgroup>
      <col class="name" />
      <col class="image" />
      <col class="state" />
      <col class="detail" />
      <col class="actions" />
    </colgroup>
    <thead>
      <tr>
        <th scope="col">Nom</th>
        <th scope="col">Image · Dossier</th>
        <th scope="col">État<span class="narrow-only"> · Détail</span></th>
        <th scope="col" class="detail-head">Détail</th>
        <th scope="col" class="actions-head">Actions</th>
      </tr>
    </thead>
    <tbody v-if="loading">
      <tr v-for="n in PLACEHOLDER_ROWS" :key="n" class="placeholder">
        <td>—</td>
        <td />
        <td />
        <td />
        <td />
      </tr>
    </tbody>
    <tbody v-else-if="groups.length === 0 && total > 0">
      <tr class="no-match-row">
        <td colspan="5">
          <FilterNoMatch
            :total="total"
            :query="query"
            @clear="emit('clearFilter')"
          />
        </td>
      </tr>
    </tbody>
    <template v-else>
      <template v-for="group in groups" :key="groupKey(group)">
        <tbody class="group-head">
          <ComposeProjectRows
            v-if="group.kind === 'project'"
            :project="group.project"
            :shown="group.containers.length"
            :expanded="expanded(group)"
            :query="query"
            :check="compose.checks[group.project.name] ?? null"
            :actions="composeActions"
            @toggle="store.toggleProject(group.project.name)"
            @forget="emit('forget', group.project)"
            @run="(action) => emit('run', group.project, action)"
            @show-tab="(id) => emit('showTab', id)"
          />
          <tr v-else class="loose-row" data-testid="loose-group">
            <td colspan="5">
              <span class="loose-title">Hors compose</span>
              <span class="caps loose-meta">
                {{ looseCount }}
                {{ looseCount > 1 ? "conteneurs" : "conteneur" }}
              </span>
              <span class="loose-meta">
                · sans étiquette com.docker.compose.project
              </span>
            </td>
          </tr>
        </tbody>
        <TransitionGroup
          v-if="expanded(group)"
          name="row-in"
          tag="tbody"
          class="group-body"
          appear
        >
          <ContainerRow
            v-for="(container, index) in group.containers"
            :key="container.id"
            :container="container"
            :query="query"
            :branch="branch(group, index)"
            :compose-run="
              group.kind === 'project'
                ? compose.running(group.project.name)
                : null
            "
            @logs="(c) => emit('logs', c)"
            @operate="(c, operation) => emit('operate', c, operation)"
          />
        </TransitionGroup>
      </template>
    </template>
  </DataTable>
</template>

<style scoped>
/* Le nom prend la largeur restante. */
.image {
  width: 22%;
}

/* Place pour « En échec · Mémoire » ou « Partiel · 12/12 ». */
.state {
  width: var(--docker-state-column-width);
}

.detail {
  width: 18%;
}

/* Cinq actions, un séparateur, et la place d'un chrono T+. */
.actions {
  width: calc(
    5 * var(--icon-button-size) + 2 * var(--sp-4) + var(--sp-3) + var(--sp-7)
  );
}

.actions-head {
  text-align: right;
}

.narrow-only {
  display: none;
}

/* Fenêtre étroite : Détail passe sous l'état, dans une colonne commune. */
@media (max-width: 1100px) {
  .detail {
    width: 0;
  }

  .detail-head {
    padding: 0;
    font-size: 0;
  }

  .narrow-only {
    display: inline;
  }
}

/* En-tête de groupe : séparé du groupe précédent par un trait marqué. */
.group-head :deep(tr) {
  background: var(--color-surface-raised);
}

.group-head:not(:first-of-type) :deep(tr:first-child td) {
  border-top: var(--border-width) solid var(--color-border-strong);
}

.group-body :deep(tr:last-child td) {
  border-bottom: var(--border-width) solid var(--color-border);
}

.loose-title {
  margin-right: var(--sp-3);
  font-family: var(--font-title);
  font-size: var(--fs-lg);
  font-style: italic;
}

.loose-meta {
  color: var(--color-text-muted);
  font-size: var(--fs-xs);
  font-weight: 400;
}
</style>
