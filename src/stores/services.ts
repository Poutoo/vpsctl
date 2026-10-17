import { ref, watch, type Ref } from "vue";
import { defineStore } from "pinia";
import * as docker from "../api/docker";
import { errorMessage } from "../api/errors";
import * as systemd from "../api/systemd";
import type { Action } from "../bindings/Action";
import type { AppError } from "../bindings/AppError";
import type { Container } from "../bindings/Container";
import type { RunState } from "../bindings/RunState";
import type { Service } from "../bindings/Service";
import { useNotify } from "../composables/useNotify";
import { useConnectionStore } from "./connection";
import {
  containerLogTarget,
  serviceLogTarget,
  useTelemetryStore,
} from "./telemetry";

/** Une liste du tableau de bord, chargée indépendamment de l'autre. */
export interface Section<T> {
  items: T[];
  /** Éléments illisibles dans la réponse du serveur, exclus de `items`. */
  skipped: number;
  error: AppError | null;
  /** Au moins un chargement terminé (réussi ou non). */
  loaded: boolean;
}

/** Action sur une ligne du tableau de bord. */
export type Operation = Action | "remove";

/** Conteneur (par ID) ou service (par unité) visé par une action. */
export type Target =
  | { kind: "docker"; id: string; name: string }
  | { kind: "systemd"; unit: string };

/** Action en cours sur une ligne : elle s'affiche « en transition ». */
export interface Pending {
  operation: Operation;
  /** Début de l'action (`performance.now()`), pour le chrono T+. */
  since: number;
}

/** Changement d'état à animer une fois, après une action. */
export interface Arrival {
  direction: "on" | "off";
  /** Change à chaque animation, pour la rejouer. */
  token: number;
}

/**
 * Délai avant de relire l'état après une action dont la commande rend la
 * main avant le résultat : toute action systemd (`--no-block`), démarrage
 * et redémarrage Docker (voir docs/03-architecture.md).
 */
const VERIFY_DELAY_MS = 2_500;
/** Durée pendant laquelle une animation de changement d'état reste armée. */
const ARRIVAL_MS = 1_000;

function emptySection<T>(): Section<T> {
  return { items: [], skipped: 0, error: null, loaded: false };
}

interface Fetched<T> {
  items: T[];
  skipped: number;
}

async function fetchContainers(): Promise<Fetched<Container>> {
  const list = await docker.listContainers();
  return { items: list.containers, skipped: list.skipped };
}

async function fetchServices(): Promise<Fetched<Service>> {
  return { items: await systemd.listServices(), skipped: 0 };
}

export function targetKey(target: Target): string {
  return target.kind === "docker"
    ? `docker:${target.id}`
    : `systemd:${target.unit}`;
}

export function containerTarget(container: Container): Target {
  return { kind: "docker", id: container.id, name: container.name };
}

export function serviceTarget(service: Service): Target {
  return { kind: "systemd", unit: service.unit };
}

function targetName(target: Target): string {
  return target.kind === "docker" ? target.name : target.unit;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Durée d'une commande, en secondes à la française : « 0,8 s ». */
function seconds(ms: number): string {
  return `${(ms / 1000).toFixed(1).replace(".", ",")} s`;
}

const SUCCESS_TITLE: Record<Target["kind"], Record<Operation, string>> = {
  docker: {
    start: "Conteneur démarré",
    stop: "Conteneur arrêté",
    restart: "Conteneur redémarré",
    remove: "Conteneur supprimé",
  },
  systemd: {
    start: "Service démarré",
    stop: "Service arrêté",
    restart: "Service redémarré",
    remove: "Service supprimé",
  },
};

const FAILURE_TITLE: Record<Operation, string> = {
  start: "Échec du démarrage",
  stop: "Échec de l'arrêt",
  restart: "Échec du redémarrage",
  remove: "Échec de la suppression",
};

export const useServicesStore = defineStore("services", () => {
  const connection = useConnectionStore();
  const telemetry = useTelemetryStore();
  const { notify } = useNotify();

  const containers = ref(emptySection<Container>()) as Ref<Section<Container>>;
  const services = ref(emptySection<Service>()) as Ref<Section<Service>>;
  /** Fin du dernier rafraîchissement (horodatage en ms). */
  const refreshedAt = ref<number | null>(null);
  const refreshing = ref(false);
  /** Actions en cours, par ligne (`targetKey`). */
  const pending = ref<Record<string, Pending>>({});
  /** Animations de changement d'état à jouer, par ligne. */
  const arrivals = ref<Record<string, Arrival>>({});

  let inFlight: Promise<void> | null = null;
  /** Incrémenté à chaque remise à zéro : les réponses antérieures sont ignorées. */
  let generation = 0;
  let arrivalToken = 0;

  /**
   * Rafraîchit les deux listes. Un appel pendant un rafraîchissement en cours
   * renvoie celui-ci : deux rafraîchissements ne se chevauchent jamais.
   */
  function refresh(): Promise<void> {
    inFlight ??= run().finally(() => {
      inFlight = null;
    });
    return inFlight;
  }

  /** Rafraîchissement lancé après cet appel (pas un rafraîchissement déjà en route). */
  async function refreshFresh(): Promise<void> {
    if (inFlight) await inFlight;
    await refresh();
  }

  async function run() {
    const current = generation;
    refreshing.value = true;
    try {
      // Chaque liste gère son propre échec : l'une n'empêche pas l'autre.
      await Promise.all([
        load(containers, fetchContainers, current),
        load(services, fetchServices, current),
      ]);
      if (current === generation) refreshedAt.value = Date.now();
    } finally {
      refreshing.value = false;
    }
  }

  async function load<T>(
    section: Ref<Section<T>>,
    fetch: () => Promise<Fetched<T>>,
    current: number,
  ) {
    let next: Section<T>;
    try {
      next = { ...(await fetch()), error: null, loaded: true };
    } catch (error) {
      next = { items: [], skipped: 0, error: error as AppError, loaded: true };
    }
    if (current === generation) section.value = next;
  }

  function reset() {
    generation++;
    containers.value = emptySection();
    services.value = emptySection();
    refreshedAt.value = null;
    pending.value = {};
    arrivals.value = {};
  }

  /** État réel d'une ligne dans la dernière liste, ou `null` si elle a disparu. */
  function find(target: Target): Container | Service | null {
    if (target.kind === "docker") {
      return containers.value.items.find((c) => c.id === target.id) ?? null;
    }
    return services.value.items.find((s) => s.unit === target.unit) ?? null;
  }

  function call(target: Target, operation: Operation): Promise<void> {
    if (target.kind === "docker") {
      return operation === "remove"
        ? docker.removeContainer(target.id)
        : docker.dockerAction(target.id, operation);
    }
    if (operation === "remove") {
      return Promise.reject({
        kind: "InvalidInput",
        message: "suppression de service non prise en charge",
      } satisfies AppError);
    }
    return systemd.serviceAction(target.unit, operation);
  }

  function openLogs(target: Target) {
    telemetry.showLogs(
      target.kind === "docker"
        ? containerLogTarget(target.id, target.name)
        : serviceLogTarget(target.unit),
    );
  }

  /**
   * Lance une action (la confirmation GO / NO GO est faite avant, par
   * l'appelant). Pendant l'action et jusqu'à la vérification, la ligne est
   * « en transition » : les rafraîchissements automatiques ne changent pas
   * son affichage. L'état est ensuite relu (après 2,5 s si la commande rend
   * la main avant le résultat) ; seulement alors il est affiché, avec une
   * notification et l'animation de changement d'état.
   */
  async function perform(target: Target, operation: Operation) {
    const key = targetKey(target);
    if (pending.value[key]) return;
    const current = generation;
    const before = find(target)?.state ?? null;
    const since = performance.now();
    pending.value[key] = { operation, since };

    try {
      await call(target, operation);
    } catch (raw) {
      if (current !== generation) return;
      delete pending.value[key];
      notify({
        tone: "error",
        title: FAILURE_TITLE[operation],
        detail: targetName(target),
        message: errorMessage(raw as AppError),
      });
      void refresh();
      return;
    }
    const duration = performance.now() - since;

    const synchronous =
      target.kind === "docker" &&
      (operation === "stop" || operation === "remove");
    if (!synchronous) await sleep(VERIFY_DELAY_MS);
    if (current !== generation) return;
    await refreshFresh();
    if (current !== generation) return;

    delete pending.value[key];
    const found = find(target);
    conclude(target, operation, before, found, duration);
  }

  function conclude(
    target: Target,
    operation: Operation,
    before: RunState | null,
    found: Container | Service | null,
    duration: number,
  ) {
    const name = targetName(target);
    const success = () =>
      notify({
        tone: "success",
        title: SUCCESS_TITLE[target.kind][operation],
        detail: `${name} · ${seconds(duration)}`,
      });

    if (operation === "remove") {
      if (found === null) success();
      return;
    }
    if (found === null) return;
    const after = found.state;
    animate(target, before, after);

    if (operation === "stop") {
      if (after === "stopped" || after === "failed") success();
      return;
    }
    // Démarrage, redémarrage : vérification de l'état réel.
    if (after === "nominal") {
      success();
    } else if (after === "failed" || after === "stopped") {
      notify({
        tone: "error",
        title: FAILURE_TITLE[operation],
        detail: `${name} · ${stateDetail(found)}`,
        action: { label: "Voir la télémétrie", run: () => openLogs(target) },
      });
    }
    // Encore en transition (service lent) : l'état réel est affiché, sans verdict.
  }

  /** Complément d'un échec : code de sortie Docker, ou état brut systemd. */
  function stateDetail(found: Container | Service): string {
    if ("dockerState" in found) {
      return found.exitCode !== null
        ? `code ${found.exitCode}`
        : found.dockerState;
    }
    return `${found.active}/${found.sub}`;
  }

  /** Arme l'animation d'allumage ou d'extinction, jouée une seule fois. */
  function animate(target: Target, before: RunState | null, after: RunState) {
    if (before === null || before === after) return;
    const direction =
      after === "nominal" ? "on" : before === "nominal" ? "off" : null;
    if (direction === null) return;
    const key = targetKey(target);
    const token = ++arrivalToken;
    arrivals.value[key] = { direction, token };
    setTimeout(() => {
      if (arrivals.value[key]?.token === token) delete arrivals.value[key];
    }, ARRIVAL_MS);
  }

  // Les listes appartiennent à la liaison : rien ne survit à une déconnexion.
  watch(
    () => connection.info?.connectedAt,
    () => reset(),
  );

  return {
    containers,
    services,
    refreshedAt,
    refreshing,
    pending,
    arrivals,
    refresh,
    perform,
  };
});
