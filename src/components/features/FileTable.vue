<script setup lang="ts">
/**
 * Contenu du dossier affiché : grille virtualisée (@tanstack/vue-virtual),
 * lignes de hauteur `--row-height`, triable par colonne, dossiers en
 * premier (le tri est fait par le store). Fluide avec des milliers
 * d'entrées : seules les lignes visibles sont rendues.
 *
 * Clavier, quand la grille a le focus (la ligne sélectionnée est désignée
 * par `aria-activedescendant`) :
 * - flèches, Début, Fin, Pg↑, Pg↓ : sélection ;
 * - Entrée : ouvre un dossier (rien sur un fichier) ;
 * - Retour arrière : dossier parent ;
 * - F2 : renommer ; Suppr : supprimer ; Ctrl+D : télécharger ;
 *   Ctrl+Maj+C : copier le chemin ;
 * - Menu ou Maj+F10 : menu de la ligne.
 *
 * Clic droit sur une ligne : menu de la ligne ; dans une zone vide : menu du
 * dossier (Nouveau dossier, Nouveau fichier, Téléverser).
 *
 * Création et renommage : saisie du nom dans la liste (`draft`), Entrée
 * valide, Échap annule. Les actions sont émises vers le parent.
 */
import {
  defaultRangeExtractor,
  useVirtualizer,
  type Range,
} from "@tanstack/vue-virtual";
import {
  ContextMenuRoot,
  ContextMenuTrigger,
  DropdownMenuRoot,
  DropdownMenuTrigger,
} from "reka-ui";
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  useId,
  watch,
} from "vue";
import type { FileEntry } from "../../bindings/FileEntry";
import { formatBytes, formatModified } from "../../format";
import {
  isDirLike,
  isFileLike,
  nameProblem,
  type SortKey,
  type SortOrder,
} from "../../remoteFiles";
import type { BusyRow } from "../../stores/files";
import AppIcon, { type AppIconName } from "../ui/AppIcon.vue";
import AppTooltip from "../ui/AppTooltip.vue";
import RunStateLabel from "../ui/RunStateLabel.vue";
import FileMenu, { type FileAction } from "./FileMenu.vue";
import FolderMenu, { type FolderAction } from "./FolderMenu.vue";

/** Saisie d'un nom dans la liste : création ou renommage. */
export type Draft =
  { mode: "create"; kind: "file" | "dir" } | { mode: "rename"; path: string };

const props = defineProps<{
  entries: FileEntry[];
  selected: string | null;
  sort: SortOrder;
  busy: Record<string, BusyRow>;
  /** Change à chaque dossier chargé : rejoue le fondu d'apparition. */
  listKey: string;
  draft: Draft | null;
  /** Refus du serveur pour le dernier nom validé, affiché sous la saisie. */
  draftError: string | null;
  /** Validation en cours : la saisie reste ouverte et ne réagit plus. */
  draftBusy: boolean;
  /** Dossier affiché, en tête du menu du dossier. */
  folder: string;
  /** Le dossier accepte une création (menu du dossier). */
  canCreate: boolean;
}>();

const emit = defineEmits<{
  select: [path: string];
  open: [entry: FileEntry];
  up: [];
  sort: [key: SortKey];
  action: [entry: FileEntry, action: FileAction];
  /** Action du menu du dossier (clic droit dans une zone vide). */
  folderAction: [action: FolderAction];
  commitDraft: [name: string];
  cancelDraft: [];
  /** Le nom saisi a changé : l'erreur du serveur n'est plus d'actualité. */
  editDraft: [];
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

/** Ligne dont le menu déroulant (« ⋯ ») est ouvert. */
const menuOpen = ref<string | null>(null);

/** Entrée en cours de renommage. */
const renamed = computed(() => {
  const draft = props.draft;
  if (draft?.mode !== "rename") return null;
  return props.entries.find((entry) => entry.path === draft.path) ?? null;
});

/**
 * Ligne gardée rendue même hors de la zone visible : celle du renommage. Sa
 * saisie, démontée par le défilement, perdrait son focus et son contenu.
 */
const pinnedIndex = computed(() =>
  renamed.value ? props.entries.indexOf(renamed.value) : -1,
);

const virtualizer = useVirtualizer(
  computed(() => {
    const pinned = pinnedIndex.value;
    return {
      count: props.entries.length,
      getScrollElement: () => scroller.value,
      estimateSize: () => rowHeight.value,
      overscan: 10,
      rangeExtractor: (range: Range) => {
        const indexes = defaultRangeExtractor(range);
        if (pinned < 0 || indexes.includes(pinned)) return indexes;
        return [...indexes, pinned].sort((a, b) => a - b);
      },
    };
  }),
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
const hintId = `${uid}-draft-hint`;

const selectedIndex = computed(() =>
  props.entries.findIndex((entry) => entry.path === props.selected),
);
/** Ligne désignée par `aria-activedescendant`, seulement si elle est rendue. */
const activeDescendant = computed(() => {
  const index = selectedIndex.value;
  return index >= 0 && visible.value.some((v) => v.row.index === index)
    ? rowId(index)
    : undefined;
});

// --- Défilement --------------------------------------------------------------

// Nouveau dossier : la liste repart du haut, ou de l'entrée sélectionnée
// (retour au dossier parent). Une relecture du même dossier ne bouge pas.
watch(
  () => props.listKey,
  () => {
    if (props.draft) return;
    const index = selectedIndex.value;
    if (index >= 0) virtualizer.value.scrollToIndex(index, { align: "center" });
    else virtualizer.value.scrollToOffset(0);
  },
  { flush: "post" },
);

// Sélection décidée ailleurs (création, renommage) ou tri changé : la ligne
// sélectionnée est ramenée à l'écran. Une relecture qui décale seulement son
// index (fin d'un envoi) ne fait pas sauter la vue.
watch(
  () => [props.selected, props.sort.key, props.sort.descending] as const,
  () => {
    const index = selectedIndex.value;
    if (index >= 0 && !props.draft) {
      virtualizer.value.scrollToIndex(index, { align: "auto" });
    }
  },
  { flush: "post" },
);

/**
 * Largeur de la barre de défilement de la zone des lignes, reportée en marge
 * droite de l'en-tête : les colonnes en pourcentage se calculent alors sur
 * la même largeur (barres classiques de WebView2 ; 0 avec des barres
 * superposées).
 */
const gutter = ref(0);

function measureGutter() {
  const element = scroller.value;
  if (element) gutter.value = element.offsetWidth - element.clientWidth;
}

let resizeObserver: ResizeObserver | null = null;
onMounted(() => {
  resizeObserver = new ResizeObserver(measureGutter);
  if (scroller.value) resizeObserver.observe(scroller.value);
  measureGutter();
});
onBeforeUnmount(() => resizeObserver?.disconnect());
watch([totalHeight, () => props.draft], measureGutter, { flush: "post" });

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
  const original = `« ${residue.original} »`;
  if (residue.kind === "part") {
    return `Fichier temporaire d'un envoi interrompu vers ${original}. Son contenu est incomplet : il peut être supprimé.`;
  }
  if (residue.originalExists) {
    return `Ancien contenu de ${original}, mis de côté pendant un écrasement interrompu. ${original} existe : vérifiez-le, puis supprimez ce résidu.`;
  }
  return `Ancien contenu de ${original}, mis de côté pendant un écrasement interrompu. ${original} n'existe plus : c'est probablement la seule copie du fichier d'origine. Renommez ce fichier en ${original} pour le récupérer.`;
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
  const key = event.key.toLowerCase();

  if (event.ctrlKey && !event.altKey && !event.metaKey) {
    if (key === "d" && !event.shiftKey) {
      if (entry && isFileLike(entry)) emit("action", entry, "download");
    } else if (key === "c" && event.shiftKey) {
      if (entry) emit("action", entry, "copyPath");
    } else {
      return;
    }
    event.preventDefault();
    return;
  }

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
    case "F2":
      if (entry) emit("action", entry, "rename");
      break;
    case "Delete":
      if (entry) emit("action", entry, "delete");
      break;
    case "ContextMenu":
      if (entry) openMenu(entry, current);
      break;
    case "F10":
      if (!event.shiftKey) return;
      if (entry) openMenu(entry, current);
      break;
    default:
      return;
  }
  event.preventDefault();
}

function onRowClick(entry: FileEntry) {
  emit("select", entry.path);
  focus();
}

function onRowDoubleClick(entry: FileEntry) {
  if (isDirLike(entry)) emit("open", entry);
}

/** Donne le focus à la grille. */
function focus() {
  grid.value?.focus();
}

defineExpose({ focus });

// --- Menus -------------------------------------------------------------------

/** Ligne visée par le menu contextuel. */
const contextEntry = ref<FileEntry | null>(null);
/** Le menu contextuel est ouvert. */
const contextOpen = ref(false);
/** Un menu est ouvert : les infobulles des lignes sont coupées (Échap ferme le menu). */
const anyMenuOpen = computed(
  () => menuOpen.value !== null || contextOpen.value,
);

/**
 * Boutons « ⋯ » rendus, qui ancrent leur menu. L'infobulle qui enveloppe le
 * bouton crée son propre contexte de positionnement (Reka UI), qui capte
 * l'ancre du menu : l'ancre est donc passée explicitement (`reference`).
 * Fonctions de référence gardées par ligne : une fonction stable n'est pas
 * rappelée avec `null` à chaque rendu.
 */
const menuButtons = new Map<string, HTMLElement>();
const menuButtonRefs = new Map<string, (element: unknown) => void>();
/** Incrémenté quand le bouton de la ligne ouverte est (re)monté. */
const anchorTick = ref(0);

function menuButtonRef(path: string) {
  let setter = menuButtonRefs.get(path);
  if (!setter) {
    setter = (element: unknown) => {
      if (element instanceof HTMLElement) {
        const changed = menuButtons.get(path) !== element;
        menuButtons.set(path, element);
        if (changed && menuOpen.value === path) anchorTick.value++;
      } else {
        menuButtons.delete(path);
      }
    };
    menuButtonRefs.set(path, setter);
  }
  return setter;
}

/**
 * Ancre du menu d'une ligne : `null` quand il est fermé, jamais `undefined`.
 * Reka UI ne transmet pas une prop `undefined` au montage, puis ne la suit
 * plus : la clé doit exister dès le départ.
 */
function menuAnchor(path: string): HTMLElement | null {
  void anchorTick.value;
  return menuOpen.value === path ? (menuButtons.get(path) ?? null) : null;
}

// Changement de dossier : les références des anciennes lignes sont oubliées.
watch(
  () => props.listKey,
  () => {
    for (const path of menuButtonRefs.keys()) {
      if (!menuButtons.has(path)) menuButtonRefs.delete(path);
    }
  },
);

/**
 * Ferme le menu déroulant sans passer par Reka UI (ligne démontée ou sortie
 * de la zone visible). Le focus revient à la grille après le rendu, une fois
 * le piège à focus du menu retiré (`onMenuClosed`).
 */
function closeMenu() {
  menuOpen.value = null;
  pending = null;
  onMenuClosed();
}

// Ligne du menu déroulant sortie du rendu (relecture, défilement).
watch(visible, (rows) => {
  const open = menuOpen.value;
  if (open !== null && !rows.some((v) => v.entry.path === open)) closeMenu();
});

/** Défilement : un menu dont le bouton sort de la zone visible est refermé. */
function onScroll() {
  const open = menuOpen.value;
  const button = open === null ? undefined : menuButtons.get(open);
  const area = scroller.value;
  if (!button || !area) return;
  const b = button.getBoundingClientRect();
  const a = area.getBoundingClientRect();
  if (b.bottom <= a.top || b.top >= a.bottom) closeMenu();
}
/** Action choisie dans un menu, lancée une fois le menu refermé. */
let pending:
  { entry: FileEntry; action: FileAction } | { folder: FolderAction } | null =
  null;

function openMenu(entry: FileEntry, index: number) {
  virtualizer.value.scrollToIndex(index, { align: "auto" });
  menuOpen.value = entry.path;
  swallowNextContextMenu();
}

/**
 * La touche Menu (et Maj+F10) émet aussi un `contextmenu`, au relâchement
 * sous Windows : il est ignoré une fois, le menu de la ligne étant déjà
 * ouvert. Sinon le menu du navigateur s'afficherait par-dessus.
 */
function swallowNextContextMenu() {
  const swallow = (event: Event) => {
    event.preventDefault();
    event.stopPropagation();
  };
  document.addEventListener("contextmenu", swallow, {
    capture: true,
    once: true,
  });
  setTimeout(
    () =>
      document.removeEventListener("contextmenu", swallow, { capture: true }),
    1000,
  );
}

/**
 * Pas de menu du navigateur dans la grille (en-tête), sauf dans un champ de
 * saisie. La zone des lignes est laissée au menu contextuel : Reka UI ne
 * l'ouvre que si l'événement n'a pas été annulé (`onContextCapture` décide).
 */
function onGridContextMenu(event: MouseEvent) {
  const target = event.target as HTMLElement;
  if (target.closest("input") || target.closest(".scroller-wrap")) return;
  event.preventDefault();
}

function choose(entry: FileEntry, action: FileAction) {
  pending = { entry, action };
}

function chooseFolder(action: FolderAction) {
  pending = { folder: action };
}

/**
 * Menu refermé : le focus revient à la grille, puis l'action choisie est
 * lancée. Le délai laisse le menu rendre son piège à focus, pour que la
 * saisie d'un nom ou une fenêtre GO / NO GO prennent le focus à leur tour.
 */
function onMenuClosed() {
  setTimeout(() => {
    const chosen = pending;
    pending = null;
    focus();
    if (chosen && "folder" in chosen) emit("folderAction", chosen.folder);
    else if (chosen) emit("action", chosen.entry, chosen.action);
  }, 0);
}

function onDropdownOpen(entry: FileEntry, open: boolean) {
  if (open) {
    menuOpen.value = entry.path;
    emit("select", entry.path);
  } else if (menuOpen.value === entry.path) {
    menuOpen.value = null;
    onMenuClosed();
  }
}

function onContextOpen(open: boolean) {
  contextOpen.value = open;
  if (!open) onMenuClosed();
}

/** Le focus est rendu par `onMenuClosed`, pas par le menu. */
function preventFocusReturn(event: Event) {
  event.preventDefault();
}

/**
 * Clic droit : sur une ligne, la sélectionne et ouvre son menu ; dans une
 * zone vide, ouvre le menu du dossier (créer, téléverser) ; dans un champ,
 * laisse le menu du système (copier, coller).
 */
function onContextCapture(event: MouseEvent) {
  const target = event.target as HTMLElement;
  if (target.closest("input")) {
    event.stopPropagation();
    return;
  }
  // Une saisie en cours ou un menu déroulant ouvert garde la main.
  if (props.draft || menuOpen.value !== null) {
    event.preventDefault();
    event.stopPropagation();
    return;
  }
  const row = target.closest<HTMLElement>("[data-index]");
  const entry = row ? props.entries[Number(row.dataset.index)] : undefined;
  contextEntry.value = entry ?? null;
  if (entry) emit("select", entry.path);
}

// --- Saisie d'un nom -----------------------------------------------------------

const draftValue = ref("");
const draftInput = ref<HTMLInputElement | null>(null);
/** Entrée a été pressée : un nom vide est alors signalé. */
const attempted = ref(false);
/** La saisie a été fermée au clavier : le focus revient à la grille. */
let refocusAfterDraft = false;
/** La saisie a été validée : l'entrée obtenue est ramenée à l'écran. */
let revealAfterDraft = false;

function setDraftInput(element: unknown) {
  draftInput.value = element instanceof HTMLInputElement ? element : null;
}

const draftProblem = computed(() => {
  const draft = props.draft;
  if (!draft) return null;
  if (props.draftError) return props.draftError;
  const name = draftValue.value;
  if (name === "" && !attempted.value) return null;
  const problem = nameProblem(name);
  if (problem) return problem;
  const clash = props.entries.some(
    (entry) =>
      entry.name === name &&
      !(draft.mode === "rename" && entry.path === draft.path),
  );
  return clash ? "Un élément porte déjà ce nom." : null;
});

const draftHint = computed(() => {
  if (props.draftBusy) return "Validation…";
  if (draftProblem.value) return draftProblem.value;
  return props.draft?.mode === "rename"
    ? "↵ renommer · Échap annuler"
    : "↵ créer · Échap annuler";
});

/** Donne le focus à la saisie ; au renommage, sélectionne le nom sans l'extension. */
function focusDraft(attempt = 0) {
  const input = draftInput.value;
  if (!input) {
    if (attempt < 3) requestAnimationFrame(() => focusDraft(attempt + 1));
    return;
  }
  input.focus();
  const entry = renamed.value;
  if (entry) {
    const dot = entry.name.lastIndexOf(".");
    const end = dot > 0 && !isDirLike(entry) ? dot : entry.name.length;
    input.setSelectionRange(0, end);
  }
}

/** Ouvre une saisie : nom de départ, ligne amenée à l'écran, focus. */
function applyDraft() {
  attempted.value = false;
  draftValue.value = renamed.value?.name ?? "";
  const index = renamed.value ? props.entries.indexOf(renamed.value) : 0;
  if (props.entries.length > 0) {
    virtualizer.value.scrollToIndex(Math.max(index, 0), { align: "auto" });
  }
  void nextTick(() => focusDraft());
}

watch(
  () => props.draft,
  (draft) => {
    if (draft) {
      applyDraft();
      return;
    }
    attempted.value = false;
    // Saisie validée : l'entrée créée ou renommée, sélectionnée, est montrée.
    // Pas après un clic ailleurs : la liste sauterait sous le pointeur.
    if (revealAfterDraft) {
      revealAfterDraft = false;
      const index = selectedIndex.value;
      if (index >= 0) virtualizer.value.scrollToIndex(index, { align: "auto" });
    }
    if (refocusAfterDraft) {
      refocusAfterDraft = false;
      void nextTick(focus);
    }
  },
);

// Liste montée avec une saisie déjà ouverte (dossier vide, puis « Nouveau
// fichier ») : la saisie prend le focus dès que la liste existe. Sinon, la
// sélection gardée (retour au parent depuis un dossier vide ou illisible,
// retour sur l'écran) est montrée. Après le rendu : le virtualiseur n'a son
// élément de défilement qu'à la fin du montage.
onMounted(() => {
  if (props.draft) {
    applyDraft();
    return;
  }
  void nextTick(() => {
    const index = selectedIndex.value;
    if (index >= 0) virtualizer.value.scrollToIndex(index, { align: "center" });
  });
});

// Refus du serveur : la saisie reprend le focus pour être corrigée.
watch(
  () => props.draftError,
  (error) => {
    if (error) void nextTick(() => focusDraft());
  },
);

function onDraftInput() {
  refocusAfterDraft = false;
  emit("editDraft");
}

function commitDraft() {
  if (props.draftBusy) return;
  attempted.value = true;
  const name = draftValue.value;
  if (nameProblem(name) || draftProblem.value) return;
  if (renamed.value && name === renamed.value.name) {
    cancelDraft();
    return;
  }
  refocusAfterDraft = true;
  revealAfterDraft = true;
  emit("commitDraft", name);
}

function cancelDraft() {
  refocusAfterDraft = true;
  revealAfterDraft = false;
  emit("cancelDraft");
}

/**
 * Clic ailleurs : la saisie est abandonnée, sauf pendant sa validation. Le
 * focus reste où l'utilisateur a cliqué.
 */
function onDraftBlur() {
  if (props.draftBusy) return;
  refocusAfterDraft = false;
  revealAfterDraft = false;
  emit("cancelDraft");
}
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
    @contextmenu="onGridContextMenu"
  >
    <div
      class="row head"
      role="row"
      aria-rowindex="1"
      :style="{ paddingRight: `${gutter}px` }"
    >
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

    <ContextMenuRoot @update:open="onContextOpen">
      <ContextMenuTrigger
        as="div"
        class="scroller-wrap"
        @contextmenu.capture="onContextCapture"
      >
        <div ref="scroller" class="scroller" @scroll.passive="onScroll">
          <div v-if="draft?.mode === 'create'" class="row draft-row" role="row">
            <div role="gridcell" class="cell draft-cell">
              <AppIcon
                class="kind"
                :class="draft.kind"
                :name="draft.kind === 'dir' ? 'folder' : 'file'"
              />
              <input
                :ref="setDraftInput"
                v-model="draftValue"
                type="text"
                class="draft-input"
                spellcheck="false"
                autocomplete="off"
                :aria-label="
                  draft.kind === 'dir'
                    ? 'Nom du nouveau dossier'
                    : 'Nom du nouveau fichier'
                "
                :aria-invalid="draftProblem !== null"
                :aria-describedby="hintId"
                :readonly="draftBusy"
                @input="onDraftInput"
                @keydown.enter.prevent="commitDraft"
                @keydown.esc.prevent.stop="cancelDraft"
                @blur="onDraftBlur"
              />
              <span
                :id="hintId"
                class="draft-hint truncate"
                :class="{ error: draftProblem !== null }"
                :title="draftHint"
                >{{ draftHint }}</span
              >
            </div>
          </div>

          <div
            :key="listKey"
            class="body"
            :style="{ height: `${totalHeight}px` }"
          >
            <div
              v-for="{ row, entry } in visible"
              :id="rowId(row.index)"
              :key="entry.path"
              class="row item"
              :class="{ selected: entry.path === selected }"
              role="row"
              :data-index="row.index"
              :aria-rowindex="row.index + 2"
              :aria-selected="entry.path === selected"
              :style="{ transform: `translateY(${row.start}px)` }"
              @click="onRowClick(entry)"
              @dblclick="onRowDoubleClick(entry)"
            >
              <div role="gridcell" class="cell name-cell">
                <AppIcon
                  class="kind"
                  :class="entry.kind"
                  :name="iconOf(entry)"
                />
                <template
                  v-if="draft?.mode === 'rename' && draft.path === entry.path"
                >
                  <input
                    :ref="setDraftInput"
                    v-model="draftValue"
                    type="text"
                    class="draft-input"
                    spellcheck="false"
                    autocomplete="off"
                    :aria-label="`Nouveau nom de ${entry.name}`"
                    :aria-invalid="draftProblem !== null"
                    :aria-describedby="hintId"
                    :readonly="draftBusy"
                    @input="onDraftInput"
                    @keydown.enter.prevent="commitDraft"
                    @keydown.esc.prevent.stop="cancelDraft"
                    @blur="onDraftBlur"
                    @click.stop
                    @dblclick.stop
                  />
                  <span
                    :id="hintId"
                    class="draft-hint truncate"
                    :class="{ error: draftProblem !== null }"
                    :title="draftHint"
                    >{{ draftHint }}</span
                  >
                </template>
                <template v-else>
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
                  <AppTooltip
                    v-if="entry.residue"
                    :text="residueHelp(entry)"
                    :disabled="anyMenuOpen"
                  >
                    <span class="caps tag residue">Résidu de transfert</span>
                  </AppTooltip>
                  <span v-if="entry.residue" class="visually-hidden">{{
                    residueHelp(entry)
                  }}</span>
                </template>
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
                {{
                  entry.modified === null ? "—" : formatModified(entry.modified)
                }}
              </div>
              <div role="gridcell" class="cell muted">
                {{ entry.permissions || "—" }}
              </div>
              <div
                role="gridcell"
                class="cell actions"
                @click.stop="emit('select', entry.path)"
                @dblclick.stop
              >
                <AppTooltip
                  v-if="isFileLike(entry)"
                  :text="`Télécharger ${entry.name}`"
                  side="top"
                  :disabled="anyMenuOpen"
                >
                  <button
                    type="button"
                    class="row-button"
                    tabindex="-1"
                    :aria-label="`Télécharger ${entry.name}`"
                    @click="emit('action', entry, 'download')"
                  >
                    <AppIcon name="download" />
                  </button>
                </AppTooltip>
                <DropdownMenuRoot
                  :open="menuOpen === entry.path"
                  @update:open="(open: boolean) => onDropdownOpen(entry, open)"
                >
                  <AppTooltip
                    :text="`Actions de ${entry.name}`"
                    side="top"
                    :disabled="anyMenuOpen"
                  >
                    <DropdownMenuTrigger as-child>
                      <button
                        :ref="menuButtonRef(entry.path)"
                        type="button"
                        class="row-button"
                        tabindex="-1"
                        :aria-label="`Actions de ${entry.name}`"
                      >
                        <AppIcon name="more" />
                      </button>
                    </DropdownMenuTrigger>
                  </AppTooltip>
                  <FileMenu
                    :entry="entry"
                    variant="dropdown"
                    :reference="menuAnchor(entry.path)"
                    @action="(action) => choose(entry, action)"
                    @close-auto-focus="preventFocusReturn"
                  />
                </DropdownMenuRoot>
              </div>
            </div>
          </div>
        </div>
      </ContextMenuTrigger>
      <FileMenu
        v-if="contextEntry"
        :entry="contextEntry"
        variant="context"
        @action="(action) => contextEntry && choose(contextEntry, action)"
        @close-auto-focus="preventFocusReturn"
      />
      <FolderMenu
        v-else
        :path="folder"
        :can-create="canCreate"
        @action="chooseFolder"
        @close-auto-focus="preventFocusReturn"
      />
    </ContextMenuRoot>
  </div>
</template>

<style scoped>
.grid {
  /* Nom : le reste. Taille, Modifié, Droits : une part, jamais moins que leur
     donnée la plus longue (« 1 023 Ko », « 12 sept. 14:02 », « drwxr-xr-x »). */
  --columns: minmax(0, 1fr) minmax(calc(10ch + 2 * var(--sp-4)), 11%)
    minmax(calc(15ch + 2 * var(--sp-4)), 16%)
    minmax(calc(11ch + 2 * var(--sp-4)), 13%)
    calc(2 * var(--icon-button-size) + var(--sp-1) + 2 * var(--sp-4));

  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
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
  white-space: nowrap;
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

/*
 * Déclencheur du menu contextuel : Reka UI le rend dans un fragment, qui ne
 * reçoit pas l'attribut de portée du composant. Sans :deep, ce style ne
 * s'appliquait pas et la liste débordait sur le pied de liste.
 */
.grid :deep(.scroller-wrap) {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
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

.row-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--icon-button-size);
  height: var(--icon-button-size);
  padding: 0;
  background: none;
  border: var(--border-width) solid transparent;
  border-radius: var(--r-1);
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
  cursor: pointer;
  transition:
    background-color var(--dur-fast) var(--ease-out),
    color var(--dur-fast) var(--ease-out);
}

.row-button:hover,
.row-button[data-state="open"] {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

/* Saisie d'un nom : ligne de création en tête de liste, ou dans la ligne. */
.draft-row {
  background: var(--color-selection);
  border-bottom: var(--border-width) solid var(--color-border);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-action);
}

.draft-cell {
  grid-column: 1 / -1;
}

/* Le champ garde une largeur utile ; l'aide prend le reste et se coupe. */
.draft-input {
  flex: 1 1 calc(3 * var(--sp-8));
  min-width: calc(2 * var(--sp-8));
  max-width: calc(7 * var(--sp-8));
  min-height: 0;
  height: calc(var(--row-height) - 2 * var(--sp-1));
  padding: 0 var(--sp-2);
  font-size: var(--fs-data);
}

.draft-hint {
  flex: 0 1 auto;
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
  font-weight: 400;
}

.draft-hint.error {
  color: var(--color-danger-text);
}
</style>
