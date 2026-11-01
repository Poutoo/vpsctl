import { ref } from "vue";
import { defineStore } from "pinia";
import * as compose from "../api/compose";
import type { ComposeAction } from "../bindings/ComposeAction";
import type { ComposeProject } from "../bindings/ComposeProject";
import type { FilesStatus } from "../bindings/FilesStatus";
import {
  COMPOSE_FAILURE_TITLE,
  COMPOSE_SUCCESS_TITLE,
} from "../composeActions";
import { useNotify } from "../composables/useNotify";
import { formatDuration } from "../format";
import { router } from "../router";
import { useConnectionStore } from "./connection";
import { useServicesStore } from "./services";
import {
  useTelemetryStore,
  type ComposeStream,
  type TelemetryTab,
} from "./telemetry";

/** Dernière commande d'un projet qui n'a pas réussi, affichée jusqu'à la suivante. */
export interface ComposeOutcome {
  action: ComposeAction;
  /** « exit 1 », « interrompue », « issue inconnue »… */
  text: string;
  /** Onglet de sa sortie, s'il est encore ouvert. */
  tabId: string | null;
}

/** Commande en cours sur un projet. */
export interface ComposeRunning {
  action: ComposeAction;
  /** Début (`performance.now()`), pour les chronos T+. */
  since: number;
  tabId: string;
}

/**
 * Projets docker compose : vérification de leur dossier et de leurs
 * fichiers (SFTP, `compose_check`), plus lente que le rafraîchissement de la
 * liste, et commandes lancées dans les onglets de l'écran Télémétrie
 * (docs/03-architecture.md).
 */
export const useComposeStore = defineStore("compose", () => {
  const connection = useConnectionStore();
  const telemetry = useTelemetryStore();
  const services = useServicesStore();
  const { notify } = useNotify();

  /** Dernière vérification réussie, par projet. */
  const checks = ref<Record<string, FilesStatus>>({});
  /** Dernière commande sans succès, par projet. */
  const failures = ref<Record<string, ComposeOutcome>>({});
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
    inFlight ??= runCheck().finally(() => {
      inFlight = null;
    });
    return inFlight;
  }

  async function runCheck() {
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

  /** Commande en cours sur un projet, ou `null`. */
  function running(project: string): ComposeRunning | null {
    const tab = telemetry.composeRunning(project);
    if (!tab) return null;
    const started = tab.session.startedAt ?? Date.now();
    return {
      action: tab.action,
      since: performance.now() - (Date.now() - started),
      tabId: tab.id,
    };
  }

  /**
   * Dernière commande sans succès d'un projet : échec ou interruption notés
   * à la fin, ou onglet coupé par un LOS (« issue inconnue »).
   */
  function outcome(project: string): ComposeOutcome | null {
    const noted = failures.value[project];
    if (noted) {
      return telemetry.tabById(noted.tabId) ? noted : { ...noted, tabId: null };
    }
    const lost = telemetry.tabs.find(
      (tab) =>
        tab.kind === "compose" &&
        tab.project === project &&
        tab.session.status === "lost",
    ) as (TelemetryTab & ComposeStream) | undefined;
    return lost
      ? { action: lost.action, text: "issue inconnue", tabId: lost.id }
      : null;
  }

  /** L'état réel est relu après chaque commande, quelle qu'en soit l'issue. */
  function reread() {
    void services.refreshFresh();
    void check();
  }

  function showOutput(tabId: string) {
    return {
      label: "Voir la sortie",
      run: () =>
        void router.push({ name: "telemetry", params: { tab: tabId } }),
    };
  }

  /** Fin d'une commande sans arrêt demandé : relecture, verdict d'après le code de sortie seul. */
  function finish(project: string, action: ComposeAction) {
    reread();
    const tab = telemetry.composeTab(project, action);
    if (!tab) return;
    const { status, exitCode, error, startedAt, endedAt } = tab.session;
    const duration =
      startedAt !== null && endedAt !== null
        ? ` · ${formatDuration(endedAt - startedAt)}`
        : "";
    if (status === "ended" && exitCode === 0) {
      notify({
        tone: "success",
        title: COMPOSE_SUCCESS_TITLE[action],
        detail: `${project} · exit 0${duration}`,
      });
      return;
    }
    const text =
      status === "ended"
        ? exitCode === null
          ? "tuée par un signal"
          : `exit ${exitCode}`
        : "flux interrompu";
    failures.value[project] = { action, text, tabId: tab.id };
    notify({
      tone: "error",
      title: COMPOSE_FAILURE_TITLE[action],
      detail: `${project} · ${text}${duration}`,
      message: status === "ended" ? undefined : (error ?? undefined),
      action: showOutput(tab.id),
    });
  }

  /**
   * Lance une commande sur un projet (la confirmation est faite avant). La
   * marque d'échec précédente disparaît. Renvoie l'onglet, ou `null` si la
   * commande n'a pas pu partir (notifiée par le store `telemetry`).
   */
  async function run(
    project: ComposeProject,
    action: ComposeAction,
  ): Promise<TelemetryTab | null> {
    if (connection.lost) return null;
    delete failures.value[project.name];
    const directory =
      action === "up" || action === "pull"
        ? (project.files?.workingDir ?? null)
        : null;
    const tab = await telemetry.runCompose(
      project.name,
      action,
      directory,
      finish,
    );
    if (!tab) void check();
    return tab;
  }

  /** Interruption demandée (après GO / NO GO) : notée, puis relecture. */
  async function stop(stream: ComposeStream) {
    if (!stream.session.active) return;
    await stream.session.stop();
    failures.value[stream.project] = {
      action: stream.action,
      text: "interrompue",
      tabId: telemetry.composeTab(stream.project, stream.action)?.id ?? null,
    };
    reread();
  }

  function reset() {
    generation++;
    checks.value = {};
    failures.value = {};
  }

  connection.on("lost", () => generation++);
  connection.on("restored", () => void check());
  connection.on("ended", reset);

  return { checks, failures, check, running, outcome, run, stop };
});
