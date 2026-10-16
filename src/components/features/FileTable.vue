<script setup lang="ts">
/**
 * Contenu du dossier affiché : grille virtualisée (@tanstack/vue-virtual),
 * lignes de hauteur `--row-height`, triable par colonne, dossiers en
 * premier (le tri est fait par le store). Fluide avec des milliers
 * d'entrées : seules les lignes visibles sont rendues.
 *
 * Clavier, quand la grille a le focus : flèches, Début, Fin, Pg↑, Pg↓
 * déplacent la sélection ; Entrée ouvre un dossier (rien sur un fichier) ;
 * Retour arrière remonte au dossier parent. La ligne sélectionnée est
 * désignée par `aria-activedescendant`, le focus reste sur la grille.
 */
import { useVirtualizer } from "@tanstack/vue-virtual";
import { computed, onMounted, ref, useId } from "vue";
import type { FileEntry } from "../../bindings/FileEntry";
import { formatBytes, formatModified } from "../../format";
import { isDirLike, type SortKey, type SortOrder } from "../../remoteFiles";
import type { BusyRow } from "../../stores/files";
import AppIcon, { type AppIconName } from "../ui/AppIcon.vue";
import AppTooltip from "../ui/AppTooltip.vue";
import RunStateLabel from "../ui/RunStateLabel.vue";

const props = defineProps<{
  entries: FileEntry[];
  selected: string | null;
  sort: SortOrder;
  busy: Record<string, BusyRow>;
  /** Change à chaque dossier chargé : rejoue le fondu d'apparition. */
  listKey: string;
}>();

const emit = defineEmits<{
  select: [path: string];
  open: [entry: FileEntry];
  up: [];
  sort: [key: SortKey];
}>();

const COLUMNS: { key: SortKey; label: string; numeric: boolean }[] = [
  { key: "name", label: "Nom", numeric: false },
  { key: "size", label: "Taille", numeric: true },
  { key: "modified", label: "Modifié", numeric: false },
  { key: "permissions", label: "Droits", numeric: false },
];

const grid = ref<HTMLElement | null>(null);
const scroller = ref<HTMLElement | null>(null);
/** Hauteur d'une ligne en px, lue dans le token `--row-height`. */
const rowHeight = ref(38);

onMounted(() => {
  const token = getComputedStyle(document.documentElement).getPropertyValue(
    "--row-height",
  );
  const parsed = Number.parseFloat(token);
  if (Number.isFinite(parsed) && parsed > 0) rowHeight.value = parsed;
});

const virtualizer = useVirtualizer(
  computed(() => ({
    count: props.entries.length,
    getScrollElement: () => scroller.value,
    estimateSize: () => rowHeight.value,
    overscan: 10,
  })),
);
/** Lignes rendues : celles qui sont visibles, et quelques-unes autour. */
const visible = computed(() =>
  virtualizer.value
    .getVirtualItems()
    .filter((row) => row.index < props.entries.length)
    .map((row) => ({ row, entry: props.entries[row.index] })),
);
const totalHeight = computed(() => virtualizer.value.getTotalSize());

const uid = useId();
const rowId = (index: number) => `${uid}-row-${index}`;

const selectedIndex = computed(() =>
  props.entries.findIndex((entry) => entry.path === props.selected),
);
const activeDescendant = computed(() =>
  selectedIndex.value >= 0 ? rowId(selectedIndex.value) : undefined,
);

function ariaSort(key: SortKey) {
  if (props.sort.key !== key) return "none";
  return props.sort.descending ? "descending" : "ascending";
}

function sortMark(key: SortKey): string {
  if (props.sort.key !== key) return "↕";
  return props.sort.descending ? "▼" : "▲";
}

function iconOf(entry: FileEntry): AppIconName {
  switch (entry.kind) {
    case "dir":
      return "folder";
    case "symlink":
      return "link";
    case "file":
      return "file";
    case "other":
      return "special";
  }
}

/** Genre lu par les lecteurs d'écran, à côté du nom. */
function kindLabel(entry: FileEntry): string {
  switch (entry.kind) {
    case "dir":
      return "dossier";
    case "file":
      return "fichier";
    case "other":
      return "fichier spécial";
    case "symlink":
      switch (entry.link?.targetKind) {
        case "dir":
          return "lien vers un dossier";
        case "file":
          return "lien vers un fichier";
        case "broken":
          return "lien cassé";
        default:
          return "lien symbolique";
      }
  }
}

function sizeOf(entry: FileEntry): string {
  return entry.kind === "file" || entry.kind === "other"
    ? formatBytes(entry.size)
    : "—";
}

/** Explication d'un résidu de transfert, en infobulle. */
function residueHelp(entry: FileEntry): string {
  const residue = entry.residue;
  if (!residue) return "";
  if (residue.kind === "part") {
    return `Fichier temporaire d'un envoi interrompu vers « ${residue.original} ». Son contenu est incomplet : il peut être supprimé.`;
  }
  if (residue.originalExists) {
    return `Ancien contenu de « ${residue.original} », mis de côté pendant un écrasement interrompu. « ${residue.original} » existe : vérifiez-le, puis supprimez ce résidu.`;
  }
  return `Ancien contenu de « ${residue.original} », mis de côté pendant un écrasement interrompu. « ${residue.original} » n'existe plus : c'est probablement la seule copie du fichier d'origine. Renommez ce fichier en « ${residue.original} » pour le récupérer.`;
}

// --- Sélection et clavier ----------------------------------------------------

function selectIndex(index: number) {
  const count = props.entries.length;
  if (count === 0) return;
  const target = Math.min(Math.max(index, 0), count - 1);
  emit("select", props.entries[target].path);
  virtualizer.value.scrollToIndex(target, { align: "auto" });
}

/** Lignes entières visibles, pour Pg↑ et Pg↓. */
function pageSize(): number {
  const height = scroller.value?.clientHeight ?? rowHeight.value;
  return Math.max(1, Math.floor(height / rowHeight.value) - 1);
}

function onKeydown(event: KeyboardEvent) {
  if (event.target instanceof HTMLInputElement) return;
  const current = selectedIndex.value;
  const entry = current >= 0 ? props.entries[current] : null;
  const last = props.entries.length - 1;
  switch (event.key) {
    case "ArrowDown":
      selectIndex(current < 0 ? 0 : current + 1);
      break;
    case "ArrowUp":
      selectIndex(current < 0 ? last : current - 1);
      break;
    case "Home":
      selectIndex(0);
      break;
    case "End":
      selectIndex(last);
      break;
    case "PageDown":
      selectIndex(Math.max(current, 0) + pageSize());
      break;
    case "PageUp":
      selectIndex(Math.max(current, 0) - pageSize());
      break;
    case "Enter":
      if (entry && isDirLike(entry)) emit("open", entry);
      break;
    case "Backspace":
      emit("up");
      break;
    default:
      return;
  }
  event.preventDefault();
}

function onRowClick(entry: FileEntry) {
  emit("select", entry.path);
  grid.value?.focus();
}

function onRowDoubleClick(entry: FileEntry) {
  if (isDirLike(entry)) emit("open", entry);
}

/** Donne le focus à la grille (après une action, un menu). */
function focus() {
  grid.value?.focus();
}

defineExpose({ focus });
</script>

<template>
  <div
    ref="grid"
    class="grid"
    role="grid"
    tabindex="0"
    aria-label="Contenu du dossier"
    :aria-rowcount="entries.length + 1"
    :aria-activedescendant="activeDescendant"
    @keydown="onKeydown"
  >
    <div class="row head" role="row" aria-rowindex="1">
      <div
        v-for="column in COLUMNS"
        :key="column.key"
        role="columnheader"
        class="cell"
        :class="{ numeric: column.numeric }"
        :aria-sort="ariaSort(column.key)"
      >
        <button
          type="button"
          class="sort"
          tabindex="-1"
          @click="emit('sort', column.key)"
        >
          {{ column.label }}
          <span class="mark" :class="{ active: sort.key === column.key }">{{
            sortMark(column.key)
          }}</span>
        </button>
      </div>
      <div role="columnheader" class="cell">
        <span class="visually-hidden">Actions</span>
      </div>
    </div>

    <div ref="scroller" class="scroller">
      <div :key="listKey" class="body" :style="{ height: `${totalHeight}px` }">
        <div
          v-for="{ row, entry } in visible"
          :id="rowId(row.index)"
          :key="entry.path"
          class="row item"
          :class="{ selected: entry.path === selected }"
          role="row"
          :aria-rowindex="row.index + 2"
          :aria-selected="entry.path === selected"
          :style="{ transform: `translateY(${row.start}px)` }"
          @click="onRowClick(entry)"
          @dblclick="onRowDoubleClick(entry)"
        >
          <div role="gridcell" class="cell name-cell">
            <AppIcon class="kind" :class="entry.kind" :name="iconOf(entry)" />
            <span class="name truncate" :title="entry.name">{{
              entry.name
            }}</span>
            <span class="visually-hidden">, {{ kindLabel(entry) }}</span>
            <span
              v-if="entry.link"
              class="target truncate"
              :class="{ broken: entry.link.targetKind === 'broken' }"
              :title="`Cible : ${entry.link.target}`"
              >→ {{ entry.link.target
              }}<template v-if="entry.link.targetKind === 'broken'">
                (cassé)</template
              ></span
            >
            <AppTooltip v-if="entry.residue" :text="residueHelp(entry)">
              <span class="caps tag residue">Résidu de transfert</span>
            </AppTooltip>
            <span v-if="entry.residue" class="visually-hidden">{{
              residueHelp(entry)
            }}</span>
            <RunStateLabel
              v-if="busy[entry.path]"
              class="busy"
              state="transition"
              :raw="busy[entry.path].label"
              :label="busy[entry.path].label"
              :since="busy[entry.path].since"
            />
          </div>
          <div role="gridcell" class="cell numeric muted">
            {{ sizeOf(entry) }}
          </div>
          <div role="gridcell" class="cell muted">
            {{ entry.modified === null ? "—" : formatModified(entry.modified) }}
          </div>
          <div role="gridcell" class="cell muted">
            {{ entry.permissions || "—" }}
          </div>
          <div role="gridcell" class="cell actions" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.grid {
  --columns: minmax(0, 1fr) 11% 16% 13%
    calc(2 * var(--icon-button-size) + var(--sp-2) + 2 * var(--sp-4));

  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--color-surface);
  font-family: var(--font-mono);
  font-size: var(--fs-data);
}

.grid:focus-visible {
  outline-offset: calc(-1 * var(--border-width-accent));
}

.row {
  display: grid;
  grid-template-columns: var(--columns);
  align-items: center;
  height: var(--row-height);
}

.head {
  flex-shrink: 0;
  background: var(--color-surface-raised);
  border-bottom: var(--border-width) solid var(--color-border);
}

.cell {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  min-width: 0;
  padding: 0 var(--sp-4);
}

.numeric {
  justify-content: flex-end;
}

.sort {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  padding: 0;
  background: none;
  border: none;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
  letter-spacing: var(--tracking-caps);
  text-transform: uppercase;
  cursor: pointer;
}

.sort:hover {
  color: var(--color-text);
}

.mark {
  color: var(--color-text-subtle);
}

.mark.active {
  color: var(--color-action);
}

.scroller {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.body {
  position: relative;
  animation: list-in var(--dur-base) var(--ease-out);
}

/* Apparition des données : fondu du contenu d'un dossier chargé. */
@keyframes list-in {
  from {
    opacity: 0;
  }
}

.item {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  border-bottom: var(--border-width) solid var(--color-border);
  cursor: default;
  transition: background-color var(--dur-fast) var(--ease-out);
}

.item:hover {
  background: var(--color-surface-hover);
}

/* Sélection : fond et trait d'action de 2 px à gauche. */
.item.selected {
  background: var(--color-selection);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-action);
}

.name-cell {
  font-weight: 500;
}

.kind {
  color: var(--color-text-muted);
}

.kind.dir {
  color: var(--color-action);
}

.name {
  flex-shrink: 1;
}

.truncate {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.target {
  flex-shrink: 2;
  color: var(--color-text-muted);
  font-weight: 400;
}

.target.broken {
  color: var(--color-danger-text);
}

.tag {
  flex-shrink: 0;
  padding: 0 var(--sp-1);
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-1);
  color: var(--color-text-muted);
}

.residue {
  border-color: var(--color-warning);
  color: var(--color-warning);
  cursor: help;
}

.busy {
  flex-shrink: 0;
}

.muted {
  color: var(--color-text-muted);
}

.actions {
  justify-content: flex-end;
  gap: var(--sp-1);
}
</style>
