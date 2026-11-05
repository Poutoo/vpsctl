import { defineStore } from "pinia";
import { computed, ref, shallowRef } from "vue";
import { errorMessage, toAppError } from "../api/errors";
import * as api from "../api/resources";
import type { AnalysisEvent } from "../bindings/AnalysisEvent";
import type { AnalysisKind } from "../bindings/AnalysisKind";
import type { DockerUsage } from "../bindings/DockerUsage";
import type { ResourceUpdate } from "../bindings/ResourceUpdate";
import type { Thresholds } from "../bindings/Thresholds";
import type { VolumeAnalysis } from "../bindings/VolumeAnalysis";
import { HISTORY_MS, type Point, type Span } from "../resourceChart";
import { navLevel } from "../resourceText";
import { useConnectionStore } from "./connection";

/** Points gardés au plus par série : 30 minutes à un relevé toutes les 5 s. */
export const MAX_POINTS = 360;

export interface Series {
  cpu: Point[];
  memory: Point[];
  load1: Point[];
  load5: Point[];
  received: Point[];
  sent: Point[];
}

export type AnalysisState =
  | { status: "idle" }
  | {
      status: "running";
      kind: AnalysisKind;
      runId: string | null;
      startedAt: number;
    }
  | { status: "failed"; kind: AnalysisKind; message: string }
  /** Liaison perdue pendant l'analyse : à relancer. */
  | { status: "lost"; kind: AnalysisKind };

/** Durée du surlignage d'un conteneur après « Voir <conteneur> ». */
export const HIGHLIGHT_MS = 3_000;

function emptySeries(): Series {
  return { cpu: [], memory: [], load1: [], load5: [], received: [], sent: [] };
}

/** Ajoute un point en gardant 30 minutes et `MAX_POINTS` au plus. */
export function pushPoint(points: Point[], point: Point): Point[] {
  const from = point.at - HISTORY_MS;
  const kept = points.filter((p) => p.at >= from && p.at < point.at);
  kept.push(point);
  return kept.length > MAX_POINTS ? kept.slice(kept.length - MAX_POINTS) : kept;
}

/**
 * Ressources du serveur (planche VII) : dernier relevé reçu du backend
 * (événement `resources-update`), historique de 30 minutes, pertes de
 * liaison (zones « sans relevé »), seuils du profil et analyse Docker.
 * Tout est vidé à la fin de la session et au changement de profil.
 */
export const useResourcesStore = defineStore("resources", () => {
  const connection = useConnectionStore();

  const update = shallowRef<ResourceUpdate | null>(null);
  const series = shallowRef<Series>(emptySeries());
  const gaps = ref<Span[]>([]);
  const thresholds = ref<Thresholds | null>(null);
  /** Écran Ressources affiché et fenêtre visible. */
  const onScreen = ref(false);
  /**
   * Liaison rétablie : CPU et réseau restent figés sur leur dernière valeur
   * jusqu'au deuxième relevé.
   */
  const resumed = ref(false);
  const analysis = ref<AnalysisState>({ status: "idle" });
  const usage = shallowRef<DockerUsage | null>(null);
  const volumes = shallowRef<VolumeAnalysis | null>(null);
  const highlighted = ref<string | null>(null);

  let listening: Promise<void> | null = null;
  /** Analyses lancées : un événement d'une analyse abandonnée est ignoré. */
  let analysisToken = 0;
  let highlightTimer: ReturnType<typeof setTimeout> | null = null;

  const profileId = computed(
    () => connection.loss?.profileId ?? connection.profileId,
  );
  const level = computed(() => navLevel(update.value, onScreen.value));

  function receive(next: ResourceUpdate) {
    if (profileId.value && next.profileId !== profileId.value) return;
    const previous = update.value;
    if (previous && previous.profileId !== next.profileId) reset();
    thresholds.value = next.thresholds;
    // Réémission du même relevé (seuils changés, répartition Docker) : pas
    // de nouveau point.
    if (previous && previous.at === next.at) {
      update.value = next;
      return;
    }
    const open = gaps.value.find((gap) => gap.to === null);
    if (open) open.to = next.at;
    const s = { ...series.value };
    const at = next.at;
    if (next.cpu.status === "ok") {
      s.cpu = pushPoint(s.cpu, { at, value: next.cpu.value });
      resumed.value = false;
    }
    if (next.memory.status === "ok") {
      s.memory = pushPoint(s.memory, { at, value: next.memory.value.percent });
    }
    if (next.load.status === "ok") {
      s.load1 = pushPoint(s.load1, { at, value: next.load.value.perCore[0] });
      s.load5 = pushPoint(s.load5, { at, value: next.load.value.perCore[1] });
    }
    if (next.network.status === "ok") {
      s.received = pushPoint(s.received, {
        at,
        value: next.network.value.received,
      });
      s.sent = pushPoint(s.sent, { at, value: next.network.value.sent });
    }
    series.value = s;
    update.value = next;
  }

  /** Écoute les relevés et relit le dernier (rechargement de la page). */
  function ensureListening(): Promise<void> {
    listening ??= api
      .onResourcesUpdate(receive)
      .then(() => api.resourcesState())
      .then((state) => {
        if (state && !update.value) receive(state);
      })
      .catch(() => undefined);
    return listening;
  }

  async function loadThresholds() {
    const id = profileId.value;
    if (!id) return;
    try {
      const loaded = await api.thresholds(id);
      if (id === profileId.value) thresholds.value = loaded;
    } catch {
      // Les seuils arriveront avec le prochain relevé.
    }
  }

  async function saveThresholds(next: Thresholds) {
    const id = profileId.value;
    if (!id) return;
    thresholds.value = await api.saveThresholds(id, next);
  }

  async function resetThresholds() {
    const id = profileId.value;
    if (!id) return;
    thresholds.value = await api.resetThresholds(id);
  }

  async function setOnScreen(visible: boolean) {
    if (onScreen.value === visible) return;
    onScreen.value = visible;
    try {
      await api.watchResources(visible);
    } catch {
      // Sans effet sur l'affichage : le relevé garde son mode.
    }
  }

  function onAnalysisEvent(token: number, event: AnalysisEvent) {
    if (token !== analysisToken) return;
    const current = analysis.value;
    if (current.status !== "running") return;
    if (event.type === "failed") {
      analysis.value = {
        status: "failed",
        kind: current.kind,
        message: event.message,
      };
      return;
    }
    if (event.result.kind === "volumes") volumes.value = event.result.volumes;
    else usage.value = event.result.usage;
    analysis.value = { status: "idle" };
  }

  async function analyze(kind: AnalysisKind) {
    if (analysis.value.status === "running") return;
    const token = ++analysisToken;
    analysis.value = {
      status: "running",
      kind,
      runId: null,
      startedAt: Date.now(),
    };
    try {
      const runId = await api.analyzeDocker(kind, (event) =>
        onAnalysisEvent(token, event),
      );
      const current = analysis.value;
      if (token === analysisToken && current.status === "running") {
        current.runId = runId;
      }
    } catch (e) {
      if (token !== analysisToken) return;
      const error = toAppError(e);
      analysis.value =
        error.kind === "LinkLost"
          ? { status: "lost", kind }
          : { status: "failed", kind, message: errorMessage(error) };
    }
  }

  /** Annule l'analyse en cours : la commande est arrêtée sur le serveur. */
  async function cancelAnalysis() {
    const current = analysis.value;
    if (current.status !== "running") return;
    analysisToken++;
    analysis.value = { status: "idle" };
    if (current.runId) {
      try {
        await api.stopAnalysis(current.runId);
      } catch {
        // Liaison perdue : le script de surveillance arrête la commande.
      }
    }
  }

  /** « Voir <conteneur> » : surligne sa ligne quelques secondes. */
  function highlight(containerId: string) {
    if (highlightTimer) clearTimeout(highlightTimer);
    highlighted.value = containerId;
    highlightTimer = setTimeout(() => {
      highlighted.value = null;
      highlightTimer = null;
    }, HIGHLIGHT_MS);
  }

  function freeze() {
    const last = update.value;
    if (last) gaps.value.push({ from: last.at, to: null });
    resumed.value = last !== null;
    const current = analysis.value;
    if (current.status === "running") {
      analysisToken++;
      analysis.value = { status: "lost", kind: current.kind };
    }
  }

  function reset() {
    update.value = null;
    series.value = emptySeries();
    gaps.value = [];
    thresholds.value = null;
    resumed.value = false;
    analysisToken++;
    analysis.value = { status: "idle" };
    usage.value = null;
    volumes.value = null;
    highlighted.value = null;
  }

  connection.on("lost", freeze);
  connection.on("ended", reset);

  return {
    update,
    series,
    gaps,
    thresholds,
    onScreen,
    resumed,
    analysis,
    usage,
    volumes,
    highlighted,
    level,
    profileId,
    receive,
    ensureListening,
    loadThresholds,
    saveThresholds,
    resetThresholds,
    setOnScreen,
    analyze,
    cancelAnalysis,
    highlight,
  };
});
