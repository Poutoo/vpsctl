import { ref, watch, type Ref } from "vue";
import { defineStore } from "pinia";
import * as docker from "../api/docker";
import * as systemd from "../api/systemd";
import type { AppError } from "../bindings/AppError";
import type { Container } from "../bindings/Container";
import type { Service } from "../bindings/Service";
import { useConnectionStore } from "./connection";

/** Une liste du tableau de bord, chargée indépendamment de l'autre. */
export interface Section<T> {
  items: T[];
  /** Éléments illisibles dans la réponse du serveur, exclus de `items`. */
  skipped: number;
  error: AppError | null;
  /** Au moins un chargement terminé (réussi ou non). */
  loaded: boolean;
}

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

export const useServicesStore = defineStore("services", () => {
  const connection = useConnectionStore();

  const containers = ref(emptySection<Container>()) as Ref<Section<Container>>;
  const services = ref(emptySection<Service>()) as Ref<Section<Service>>;
  /** Fin du dernier rafraîchissement (horodatage en ms). */
  const refreshedAt = ref<number | null>(null);
  const refreshing = ref(false);

  let inFlight: Promise<void> | null = null;
  /** Incrémenté à chaque remise à zéro : les réponses antérieures sont ignorées. */
  let generation = 0;

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
  }

  // Les listes appartiennent à la liaison : rien ne survit à une déconnexion.
  watch(
    () => connection.info?.connectedAt,
    () => reset(),
  );

  return { containers, services, refreshedAt, refreshing, refresh };
});
