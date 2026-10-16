import { computed, ref, watch } from "vue";
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
}

export type TransferSpec = Pick<
  Transfer,
  "direction" | "name" | "localPath" | "remotePath" | "overwrite"
>;

/** Fenêtre de calcul du débit. */
const RATE_WINDOW_MS = 3_000;

/**
 * File d'attente des transferts : un seul transfert actif à la fois, le
 * suivant démarre à la fin du précédent. Le store survit au changement
 * d'écran : un transfert continue pendant la navigation.
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
    });
    pump();
  }

  /** Démarre le premier transfert en attente si aucun n'est actif. */
  function pump() {
    if (items.value.some((item) => item.status === "active")) return;
    const next = items.value.find((item) => item.status === "queued");
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
    });
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

  // Liaison coupée ou changée : le backend a annulé les transferts en
  // cours ; une nouvelle liaison repart d'une file vide.
  watch(
    () => connection.info?.connectedAt ?? null,
    (connectedAt) => {
      if (connectedAt !== null) {
        items.value = [];
        return;
      }
      for (const item of items.value) {
        if (item.status === "queued" || item.status === "active") {
          Object.assign(item, {
            status: "cancelled",
            finishedAt: Date.now(),
            transferId: null,
          });
        }
      }
    },
  );

  return { items, counts, enqueue, cancel, remove, retry, reveal };
});
