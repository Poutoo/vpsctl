import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { errorMessage, toAppError } from "../api/errors";
import * as api from "../api/files";
import { revealLocalFile } from "../api/local";
import type { AppError } from "../bindings/AppError";
import type { Progress } from "../bindings/Progress";
import { useNotify } from "../composables/useNotify";
import { parentPath } from "../remoteFiles";
import { useConnectionStore } from "./connection";
import { useFilesStore } from "./files";

export type TransferDirection = "upload" | "download";
export type TransferStatus =
  "queued" | "active" | "done" | "cancelled" | "error";

export interface Transfer {
  key: number;
  direction: TransferDirection;
  /** Nom du fichier transféré. */
  name: string;
  localPath: string;
  remotePath: string;
  /** Envoi : remplacer la cible existante (confirmé par un GO). */
  overwrite: boolean;
  status: TransferStatus;
  transferred: number;
  /** Taille totale, connue au premier événement de progression. */
  total: number | null;
  /** Débit lissé sur les dernières secondes, en octets par seconde. */
  rate: number | null;
  error: AppError | null;
  /** Horodatages `Date.now()`. */
  startedAt: number | null;
  finishedAt: number | null;
  /** Identifiant du backend, tant que le transfert est actif. */
  transferId: string | null;
  /** Annulation demandée avant que l'identifiant soit connu. */
  cancelRequested: boolean;
  /** En attente au moment d'un LOS : ne repart que par « Reprendre la file ». */
  held: boolean;
}

export type TransferSpec = Pick<
  Transfer,
  "direction" | "name" | "localPath" | "remotePath" | "overwrite"
>;

/** Fenêtre de calcul du débit. */
const RATE_WINDOW_MS = 3_000;

/** Échec dû à la perte de liaison : signalé par le LOS, pas par une notification. */
export function cutByLoss(item: Transfer): boolean {
  return item.status === "error" && item.error?.kind === "LinkLost";
}

/**
 * File d'attente des transferts : un seul transfert actif à la fois, le
 * suivant démarre à la fin du précédent. Le store survit au changement
 * d'écran : un transfert continue pendant la navigation. Après un LOS, rien
 * ne repart seul : le transfert coupé se relance depuis le début
 * (« Réessayer »), la file retenue par « Reprendre la file ».
 */
export const useTransfersStore = defineStore("transfers", () => {
  const connection = useConnectionStore();
  const files = useFilesStore();
  const { notify } = useNotify();

  const items = ref<Transfer[]>([]);
  let nextKey = 1;
  /** Mesures récentes du transfert actif, pour le débit. */
  let samples: { at: number; bytes: number }[] = [];

  const counts = computed(() => {
    const result = { queued: 0, active: 0, done: 0, cancelled: 0, error: 0 };
    for (const item of items.value) result[item.status] += 1;
    return result;
  });

  function find(key: number): Transfer | undefined {
    return items.value.find((item) => item.key === key);
  }

  function enqueue(spec: TransferSpec) {
    items.value.push({
      ...spec,
      key: nextKey++,
      status: "queued",
      transferred: 0,
      total: null,
      rate: null,
      error: null,
      startedAt: null,
      finishedAt: null,
      transferId: null,
      cancelRequested: false,
      held: false,
    });
    pump();
  }

  const held = computed(() =>
    items.value.some((item) => item.status === "queued" && item.held),
  );

  /** Démarre le premier transfert en attente si aucun n'est actif. */
  function pump() {
    if (connection.lost) return;
    if (items.value.some((item) => item.status === "active")) return;
    const next = items.value.find(
      (item) => item.status === "queued" && !item.held,
    );
    if (next) void start(next.key);
  }

  async function start(key: number) {
    const item = find(key);
    if (!item) return;
    Object.assign(item, {
      status: "active",
      transferred: 0,
      rate: null,
      error: null,
      startedAt: Date.now(),
      finishedAt: null,
      transferId: null,
      cancelRequested: false,
    });
    samples = [];
    const onProgress = (event: Progress) => handle(key, event);
    try {
      const id =
        item.direction === "upload"
          ? await api.upload(
              item.localPath,
              item.remotePath,
              item.overwrite,
              onProgress,
            )
          : await api.download(item.remotePath, item.localPath, onProgress);
      const current = find(key);
      if (current?.status !== "active") return;
      current.transferId = id;
      if (current.cancelRequested) await api.cancelTransfer(id);
    } catch (raw) {
      finish(key, "error", toAppError(raw));
    }
  }

  /** Événement du backend (les événements peuvent précéder l'identifiant). */
  function handle(key: number, event: Progress) {
    const item = find(key);
    if (item?.status !== "active") return;
    switch (event.type) {
      case "progress":
        item.transferred = event.transferred;
        item.total = event.total;
        item.rate = measureRate(event.transferred);
        break;
      case "done":
        item.transferred = item.total ?? item.transferred;
        finish(key, "done");
        break;
      case "cancelled":
        finish(key, "cancelled");
        break;
      case "error":
        finish(key, "error", event.error);
        break;
    }
  }

  function measureRate(bytes: number): number | null {
    const at = performance.now();
    samples.push({ at, bytes });
    samples = samples.filter((sample) => at - sample.at <= RATE_WINDOW_MS);
    const first = samples[0];
    const span = (at - first.at) / 1000;
    return span >= 0.5 ? (bytes - first.bytes) / span : null;
  }

  function finish(
    key: number,
    status: "done" | "cancelled" | "error",
    error: AppError | null = null,
  ) {
    const item = find(key);
    if (item?.status !== "active") return;
    Object.assign(item, {
      status,
      error,
      finishedAt: Date.now(),
      transferId: null,
    });
    // Coupé par un LOS : le bandeau le signale, la file attend.
    if (error?.kind === "LinkLost") return;
    if (item.direction === "upload" && status !== "cancelled") {
      // Envoi terminé, ou temporaire éventuel à faire apparaître.
      files.refreshIfShowing(parentPath(item.remotePath) ?? "/");
    }
    if (status === "done") {
      notify({
        tone: "success",
        title: item.direction === "upload" ? "Envoyé" : "Téléchargé",
        detail: item.name,
        action:
          item.direction === "download"
            ? { label: "Afficher", run: () => void reveal(item.key) }
            : undefined,
      });
    } else if (status === "error" && error) {
      notify({
        tone: "error",
        title:
          item.direction === "upload"
            ? "Envoi interrompu"
            : "Téléchargement interrompu",
        detail: item.name,
        message: errorMessage(error),
      });
    }
    pump();
  }

  /** Annule le transfert actif ; rend la main une fois le nettoyage fait. */
  async function cancel(key: number) {
    const item = find(key);
    if (item?.status !== "active") return;
    item.cancelRequested = true;
    if (item.transferId !== null) await api.cancelTransfer(item.transferId);
  }

  /** Retire un transfert qui n'est pas actif. */
  function remove(key: number) {
    const index = items.value.findIndex((item) => item.key === key);
    if (index >= 0 && items.value[index].status !== "active") {
      items.value.splice(index, 1);
    }
  }

  /** Remet en attente un transfert annulé ou en erreur. */
  function retry(key: number, overwrite?: boolean) {
    if (connection.lost) return;
    const item = find(key);
    if (!item || (item.status !== "cancelled" && item.status !== "error")) {
      return;
    }
    Object.assign(item, {
      status: "queued",
      transferred: 0,
      total: null,
      rate: null,
      error: null,
      finishedAt: null,
      overwrite: overwrite ?? item.overwrite,
      held: false,
    });
    pump();
  }

  /** « Reprendre la file » : les transferts retenus par un LOS repartent. */
  function resumeQueue() {
    if (connection.lost) return;
    for (const item of items.value) item.held = false;
    pump();
  }

  /** Ouvre le dossier local d'un fichier téléchargé. */
  async function reveal(key: number) {
    const item = find(key);
    if (!item) return;
    try {
      await revealLocalFile(item.localPath);
    } catch (raw) {
      notify({
        tone: "error",
        title: "Affichage impossible",
        detail: item.localPath,
        message: errorMessage(toAppError(raw)),
      });
    }
  }

  /**
   * LOS : le transfert actif échoue (le backend l'a arrêté, son temporaire
   * distant reste en résidu), ceux en attente sont retenus.
   */
  function freeze() {
    for (const item of items.value) {
      if (item.status === "active") {
        Object.assign(item, {
          status: "error",
          error: { kind: "LinkLost" },
          rate: null,
          finishedAt: Date.now(),
          transferId: null,
        });
      } else if (item.status === "queued") {
        item.held = true;
      }
    }
  }

  connection.on("lost", freeze);
  // Fin de session : le backend a annulé les transferts, la file est vidée.
  connection.on("ended", () => (items.value = []));

  return {
    items,
    counts,
    held,
    enqueue,
    cancel,
    remove,
    retry,
    resumeQueue,
    reveal,
  };
});
