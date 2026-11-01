import { ref } from "vue";
import { defineStore } from "pinia";
import * as compose from "../api/compose";
import type { FilesStatus } from "../bindings/FilesStatus";
import { useConnectionStore } from "./connection";

/**
 * Projets docker compose : vérification de leur dossier et de leurs
 * fichiers (SFTP, `compose_check`), plus lente que le rafraîchissement de la
 * liste (docs/03-architecture.md).
 */
export const useComposeStore = defineStore("compose", () => {
  const connection = useConnectionStore();

  /** Dernière vérification réussie, par projet. */
  const checks = ref<Record<string, FilesStatus>>({});
  let inFlight: Promise<void> | null = null;
  /** Incrémenté au LOS et à la fin de session : les réponses antérieures sont ignorées. */
  let generation = 0;

  /**
   * Vérifie tous les projets. Un appel pendant une vérification en cours
   * renvoie celle-ci. Un échec garde la vérification précédente, sans
   * notification : l'action la refait de toute façon avant de partir.
   */
  function check(): Promise<void> {
    if (connection.lost) return Promise.resolve();
    inFlight ??= run().finally(() => {
      inFlight = null;
    });
    return inFlight;
  }

  async function run() {
    const current = generation;
    try {
      const list = await compose.checkProjects();
      if (current !== generation) return;
      checks.value = Object.fromEntries(
        list.map((item) => [item.project, item.status]),
      );
    } catch {
      // Vérification précédente gardée.
    }
  }

  function reset() {
    generation++;
    checks.value = {};
  }

  connection.on("lost", () => generation++);
  connection.on("restored", () => void check());
  connection.on("ended", reset);

  return { checks, check };
});
