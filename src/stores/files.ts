import { computed, ref, watch } from "vue";
import { defineStore } from "pinia";
import type { RouteLocationRaw } from "vue-router";
import * as api from "../api/files";
import type { AppError } from "../bindings/AppError";
import type { FileEntry } from "../bindings/FileEntry";
import { pathSegments, sortEntries, type SortOrder } from "../remoteFiles";
import { useConnectionStore } from "./connection";

/** Opération en cours sur une ligne : libellé et début (chrono T+). */
export interface BusyRow {
  label: string;
  since: number;
}

/** Route d'un dossier distant (`/files` = racine). */
export function filesRoute(path: string): RouteLocationRaw {
  return { name: "files", params: { path: pathSegments(path) } };
}

/**
 * Explorateur : dossier affiché, son contenu, le tri et la sélection. Le
 * dossier personnel est lu une fois par liaison ; le dernier dossier visité
 * est rouvert en revenant sur l'écran.
 */
export const useFilesStore = defineStore("files", () => {
  const connection = useConnectionStore();

  const home = ref<string | null>(null);
  /** Dossier demandé (affiché dans l'itinéraire dès la demande). */
  const path = ref<string | null>(null);
  /** Dossier dont `entries` est le contenu. */
  const loadedPath = ref<string | null>(null);
  const entries = ref<FileEntry[]>([]);
  const error = ref<AppError | null>(null);
  /** Lecture en cours depuis (`performance.now()`), sinon `null`. */
  const loadingSince = ref<number | null>(null);
  const sort = ref<SortOrder>({ key: "name", descending: false });
  /** Chemin de l'entrée sélectionnée. */
  const selected = ref<string | null>(null);
  const busy = ref<Record<string, BusyRow>>({});

  const sorted = computed(() => sortEntries(entries.value, sort.value));
  const loading = computed(() => loadingSince.value !== null);

  /** Destination de l'entrée « Fichiers » de la navigation. */
  const entryRoute = computed<RouteLocationRaw>(() => {
    const target = path.value ?? home.value;
    return target === null
      ? { name: "files", query: { depart: "personnel" } }
      : filesRoute(target);
  });

  let request = 0;
  let homeRequest: Promise<string> | null = null;

  /** Dossier personnel, lu une fois par liaison. */
  function ensureHome(): Promise<string> {
    if (home.value !== null) return Promise.resolve(home.value);
    homeRequest ??= api
      .homeDir()
      .then((value) => (home.value = value))
      .finally(() => (homeRequest = null));
    return homeRequest;
  }

  /**
   * Lit un dossier. `select` : entrée à sélectionner après la lecture (par
   * défaut, la sélection est gardée si l'entrée existe encore).
   */
  async function open(target: string, select?: string | null) {
    const current = ++request;
    if (select !== undefined) selected.value = select;
    else if (target !== path.value) selected.value = null;
    path.value = target;
    error.value = null;
    loadingSince.value = performance.now();
    try {
      const list = await api.listDir(target);
      if (current !== request) return;
      entries.value = list;
      if (!list.some((entry) => entry.path === selected.value)) {
        selected.value = null;
      }
    } catch (raw) {
      if (current !== request) return;
      error.value = raw as AppError;
      entries.value = [];
      selected.value = null;
    } finally {
      if (current === request) {
        loadedPath.value = target;
        loadingSince.value = null;
      }
    }
  }

  function refresh(select?: string | null) {
    if (path.value !== null) return open(path.value, select);
    return Promise.resolve();
  }

  /** Relit le dossier s'il est affiché (fin d'un envoi, par exemple). */
  function refreshIfShowing(dir: string) {
    if (path.value === dir && !loading.value) void refresh();
  }

  function setSort(key: SortOrder["key"]) {
    sort.value =
      sort.value.key === key
        ? { key, descending: !sort.value.descending }
        : { key, descending: false };
  }

  function markBusy(target: string, label: string) {
    busy.value = {
      ...busy.value,
      [target]: { label, since: performance.now() },
    };
  }

  function clearBusy(target: string) {
    const rest = { ...busy.value };
    delete rest[target];
    busy.value = rest;
  }

  function reset() {
    request++;
    home.value = null;
    path.value = null;
    loadedPath.value = null;
    entries.value = [];
    error.value = null;
    loadingSince.value = null;
    selected.value = null;
    busy.value = {};
  }

  // Nouvelle liaison (ou déconnexion) : le serveur a pu changer.
  watch(
    () => connection.info?.connectedAt ?? null,
    () => reset(),
  );

  return {
    home,
    path,
    loadedPath,
    entries,
    sorted,
    error,
    loading,
    loadingSince,
    sort,
    selected,
    busy,
    entryRoute,
    ensureHome,
    open,
    refresh,
    refreshIfShowing,
    setSort,
    markBusy,
    clearBusy,
  };
});
