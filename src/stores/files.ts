import { computed, ref } from "vue";
import { defineStore } from "pinia";
import type { RouteLocationRaw } from "vue-router";
import * as api from "../api/files";
import type { AppError } from "../bindings/AppError";
import type { FileEntry } from "../bindings/FileEntry";
import { filterItems } from "../listFilter";
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
  /** Fin de la dernière lecture réussie du dossier affiché (horodatage en ms). */
  const loadedAt = ref<number | null>(null);
  /** Dernière lecture réussie, montrée figée pendant un LOS. */
  let lastGood: { path: string; entries: FileEntry[]; at: number } | null =
    null;

  /**
   * Filtre du dossier affiché : vidé quand on change de dossier et à la fin
   * de la session, gardé en changeant d'écran et pendant un LOS.
   */
  const filter = ref("");

  const sorted = computed(() => sortEntries(entries.value, sort.value));
  /** Entrées triées puis filtrées, telles que la liste les affiche. */
  const shown = computed(() =>
    filterItems(sorted.value, filter.value, (entry) => [entry.name]),
  );
  const loading = computed(() => loadingSince.value !== null);
  /** Ouverture d'un autre dossier : la liste affichée est celle du précédent. */
  const navigating = computed(
    () => loading.value && loadedPath.value !== path.value,
  );

  /** Destination de l'entrée « Fichiers » de la navigation. */
  const entryRoute = computed<RouteLocationRaw>(() => {
    const target = path.value ?? home.value;
    return target === null
      ? { name: "files", query: { depart: "personnel" } }
      : filesRoute(target);
  });

  let request = 0;
  let homeRequest: Promise<string> | null = null;
  /** Relecture demandée pendant une lecture du même dossier, faite à sa fin. */
  let again = false;

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
    if (connection.lost) return;
    const current = ++request;
    if (target !== path.value) {
      again = false;
      filter.value = "";
    }
    if (select !== undefined) selected.value = select;
    else if (target !== path.value) selected.value = null;
    path.value = target;
    error.value = null;
    loadingSince.value = performance.now();
    try {
      const list = await api.listDir(target);
      if (current !== request) return;
      entries.value = list;
      loadedAt.value = Date.now();
      lastGood = { path: target, entries: list, at: loadedAt.value };
      if (!list.some((entry) => entry.path === selected.value)) {
        selected.value = null;
      }
    } catch (raw) {
      if (current !== request) return;
      if ((raw as AppError).kind === "LinkLost") return;
      error.value = raw as AppError;
      entries.value = [];
      selected.value = null;
    } finally {
      if (current === request) {
        loadedPath.value = target;
        loadingSince.value = null;
        if (again) {
          again = false;
          void open(target);
        }
      }
    }
  }

  function refresh(select?: string | null) {
    if (path.value !== null) return open(path.value, select);
    return Promise.resolve();
  }

  /**
   * Relit le dossier s'il est affiché (fin d'un envoi, par exemple). Une
   * lecture en cours a pu partir avant le changement : une relecture est
   * faite à sa fin (une seule pour une rafale d'envois), sans l'interrompre.
   */
  function refreshIfShowing(dir: string) {
    if (path.value !== dir) return;
    if (loading.value) again = true;
    else void refresh();
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
    again = false;
    lastGood = null;
    loadedAt.value = null;
    home.value = null;
    path.value = null;
    loadedPath.value = null;
    entries.value = [];
    error.value = null;
    loadingSince.value = null;
    selected.value = null;
    busy.value = {};
    filter.value = "";
  }

  /**
   * LOS : les lectures en cours sont abandonnées ; le dossier affiché garde
   * sa dernière lecture réussie, figée.
   */
  function freeze() {
    request++;
    again = false;
    loadingSince.value = null;
    busy.value = {};
    if (lastGood && lastGood.path === path.value) {
      entries.value = lastGood.entries;
      loadedPath.value = lastGood.path;
      loadedAt.value = lastGood.at;
      error.value = null;
    }
  }

  /** Heure des données figées pendant un LOS. */
  const frozenAt = computed(() => (connection.lost ? loadedAt.value : null));

  connection.on("lost", freeze);
  connection.on("restored", () => void refresh());
  // Fin de session : le serveur suivant a pu changer.
  connection.on("ended", reset);

  return {
    home,
    path,
    loadedPath,
    entries,
    sorted,
    filter,
    shown,
    error,
    loading,
    navigating,
    loadingSince,
    sort,
    selected,
    busy,
    frozenAt,
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
