import { computed, ref, shallowRef } from "vue";
import { defineStore } from "pinia";
import * as composeApi from "../api/compose";
import * as logs from "../api/logs";
import * as scriptsApi from "../api/scripts";
import type { ComposeAction } from "../bindings/ComposeAction";
import type { LogSource } from "../bindings/LogSource";
import type { Script } from "../bindings/Script";
import {
  createLogSession,
  type LogSession,
  type StreamDriver,
} from "../composables/logSession";
import { useNotify } from "../composables/useNotify";
import { formatDuration } from "../format";
import { router } from "../router";
import { useConnectionStore } from "./connection";
import { useScriptsStore } from "./scripts";

/** Lignes d'historique demandées à l'ouverture d'un flux de logs. */
const LOG_TAIL = 200;

/** Source de logs : conteneur Docker ou service systemd. */
export interface LogTarget {
  source: LogSource;
  kind: "docker" | "systemd";
  /** Nom du conteneur ou de l'unité. */
  name: string;
  /** Commande équivalente, affichée dans l'en-tête de la console. */
  command: string;
}

export function containerLogTarget(id: string, name: string): LogTarget {
  return {
    source: { docker: id },
    kind: "docker",
    name,
    command: `docker logs -f ${name}`,
  };
}

export function serviceLogTarget(unit: string): LogTarget {
  return {
    source: { systemd: unit },
    kind: "systemd",
    name: unit,
    command: `journalctl --user -fu ${unit}`,
  };
}

function logKey(target: LogTarget): string {
  return `logs:${JSON.stringify(target.source)}`;
}

function logDriver(source: LogSource): StreamDriver {
  return {
    open: (onEvent) => logs.startLogs(source, LOG_TAIL, onEvent),
    resume: (onEvent) => logs.startLogs(source, 0, onEvent),
    close: logs.stopLogs,
  };
}

/** Flux de logs affiché dans le panneau ancré ou dans un onglet. */
export interface LogsStream {
  kind: "logs";
  /** Identifie la source : un seul flux par source. */
  key: string;
  target: LogTarget;
  session: LogSession;
}

/** Exécution d'un script, toujours dans un onglet. */
export interface ScriptStream {
  kind: "script";
  /** Un seul onglet par script : relancer réutilise le sien. */
  key: string;
  /** Script tel qu'à sa dernière exécution lancée depuis cet onglet. */
  script: Script;
  session: LogSession;
}

function scriptKey(id: string): string {
  return `script:${id}`;
}

function scriptDriver(id: string): StreamDriver {
  return {
    open: (onEvent) => scriptsApi.runScript(id, onEvent),
    close: scriptsApi.stopScript,
  };
}

/** Commande docker compose sur un projet, toujours dans un onglet. */
export interface ComposeStream {
  kind: "compose";
  /** Un onglet par verbe et par projet : relancer la même commande réutilise le sien. */
  key: string;
  project: string;
  action: ComposeAction;
  /** Commande exacte renvoyée par le backend ; `null` avant le lancement. */
  command: string | null;
  /** Dossier du projet (Démarrer, Récupérer) ; `null` : par le seul nom. */
  directory: string | null;
  session: LogSession;
}

/** Onglets de commandes compose gardés au plus : le plus ancien terminé sort. */
export const MAX_COMPOSE_TABS = 6;

function composeKey(project: string, action: ComposeAction): string {
  return `compose:${project}:${action}`;
}

/** Flux affiché par une console : panneau ancré ou onglet. */
export type TelemetryStream = LogsStream | ScriptStream | ComposeStream;

/** Onglet de l'écran Télémétrie. */
export type TelemetryTab = TelemetryStream & {
  /** Identifiant local, dans l'URL (`/telemetry/:tab`). */
  id: string;
};

/** Nom d'un flux : conteneur, unité ou script. */
export function streamName(stream: TelemetryStream): string {
  switch (stream.kind) {
    case "logs":
      return stream.target.name;
    case "script":
      return stream.script.name;
    case "compose":
      return `${stream.action} · ${stream.project}`;
  }
}

/** Début (`Date.now()`) d'une commande compose en cours, pour son chrono T+ ; sinon `null`. */
export function composeRunningSince(stream: TelemetryStream): number | null {
  return stream.kind === "compose" && stream.session.active
    ? stream.session.startedAt
    : null;
}

/** Section de la navigation dont vient un flux, surlignée sur l'écran Télémétrie. */
export function streamOrigin(stream: TelemetryStream): "dashboard" | "scripts" {
  return stream.kind === "script" ? "scripts" : "dashboard";
}

/**
 * Tous les flux de l'interface (docs/03-architecture.md) :
 * - le **panneau ancré** du tableau de bord, une source à la fois, arrêté
 *   quand on quitte le tableau de bord (`useDockedStream`) ;
 * - les **onglets** de l'écran Télémétrie, qui vivent jusqu'à leur
 *   fermeture, la déconnexion ou la fermeture de l'application, quel que
 *   soit l'écran affiché.
 */
export const useTelemetryStore = defineStore("telemetry", () => {
  const connection = useConnectionStore();
  const scripts = useScriptsStore();
  const { notify } = useNotify();

  const dock = shallowRef<LogsStream | null>(null);
  const tabs = ref<TelemetryTab[]>([]);
  let nextTabId = 1;

  function tabById(id: string | null | undefined): TelemetryTab | null {
    return tabs.value.find((tab) => tab.id === id) ?? null;
  }

  function tabByKey(key: string): TelemetryTab | null {
    return tabs.value.find((tab) => tab.key === key) ?? null;
  }

  function addTab(stream: TelemetryStream): TelemetryTab {
    const tab: TelemetryTab = { ...stream, id: String(nextTabId++) };
    tabs.value.push(tab);
    return tab;
  }

  // --- Panneau ancré ---------------------------------------------------------

  /** Ouvre le panneau sur une source ; déjà affichée, rien ne change. */
  function openDock(target: LogTarget) {
    const key = logKey(target);
    if (dock.value?.key === key) return;
    void dock.value?.session.stop();
    const session = createLogSession(logDriver(target.source));
    dock.value = { kind: "logs", key, target, session };
    void session.start();
  }

  /** Ferme le panneau et arrête son flux. */
  function closeDock() {
    void dock.value?.session.stop();
    dock.value = null;
  }

  /** Tableau de bord quitté : le flux du panneau s'arrête, sa source reste choisie. */
  function suspendDock() {
    void dock.value?.session.stop();
  }

  /** Retour sur le tableau de bord : le panneau rouvre son flux. */
  function resumeDock() {
    const session = dock.value?.session;
    if (session && !session.active && session.status !== "lost") {
      void session.replay();
    }
  }

  /**
   * « Plein écran » : le flux du panneau passe dans un nouvel onglet, sans
   * redémarrer ni perdre la mémoire tampon. Renvoie l'onglet.
   */
  function promoteDock(): TelemetryTab | null {
    const stream = dock.value;
    if (stream === null) return null;
    dock.value = null;
    return addTab(stream);
  }

  /**
   * Logs d'un conteneur ou d'un service : l'onglet existant de cette source,
   * sinon le panneau ancré du tableau de bord.
   */
  function showLogs(target: LogTarget) {
    const tab = tabByKey(logKey(target));
    if (tab) {
      void router.push({ name: "telemetry", params: { tab: tab.id } });
      return;
    }
    openDock(target);
    if (router.currentRoute.value.name !== "dashboard") {
      void router.push({ name: "dashboard" });
    }
  }

  // --- Onglets ---------------------------------------------------------------

  /**
   * Ferme un onglet et arrête son flux (la confirmation est faite avant).
   * Renvoie l'onglet voisin à afficher à sa place, ou `null`.
   */
  function closeTab(id: string): TelemetryTab | null {
    const index = tabs.value.findIndex((tab) => tab.id === id);
    if (index < 0) return null;
    void tabs.value[index].session.stop();
    tabs.value.splice(index, 1);
    return tabs.value[Math.min(index, tabs.value.length - 1)] ?? null;
  }

  // --- Scripts ---------------------------------------------------------------

  /** Onglet d'un script, ouvert par une exécution. */
  function scriptTab(id: string): TelemetryTab | null {
    return tabByKey(scriptKey(id));
  }

  /**
   * Fin d'une exécution sans arrêt demandé : sa dernière exécution est
   * relue (déjà enregistrée par le backend) et une notification donne le
   * verdict ; un échec reste affiché, avec un lien vers la sortie.
   */
  function onScriptFinish(id: string) {
    void scripts.load().catch(() => undefined);
    const tab = scriptTab(id);
    if (tab?.kind !== "script") return;
    const { status, exitCode, error, startedAt, endedAt } = tab.session;
    const duration =
      startedAt !== null && endedAt !== null
        ? ` · ${formatDuration(endedAt - startedAt)}`
        : "";
    const showOutput = {
      label: "Voir la sortie",
      run: () =>
        void router.push({ name: "telemetry", params: { tab: tab.id } }),
    };
    if (status === "ended" && exitCode === 0) {
      notify({
        tone: "success",
        title: "Procédure terminée",
        detail: `${tab.script.name} · exit 0${duration}`,
      });
    } else if (status === "ended") {
      notify({
        tone: "error",
        title: "Procédure en échec",
        detail: `${tab.script.name} · ${exitCode === null ? "tuée par un signal" : `exit ${exitCode}`}${duration}`,
        action: showOutput,
      });
    } else {
      notify({
        tone: "error",
        title: "Procédure interrompue",
        detail: tab.script.name,
        message: error ?? undefined,
        action: showOutput,
      });
    }
  }

  /**
   * Exécute un script (la confirmation GO / NO GO est faite avant) dans son
   * onglet, créé au besoin ; déjà en cours, l'onglet est simplement
   * renvoyé. Échec du lancement (limite de flux, liaison absente) :
   * notification, et l'onglet créé pour l'occasion est retiré.
   */
  async function runScript(script: Script): Promise<TelemetryTab | null> {
    const existing = scriptTab(script.id);
    if (existing?.session.active) return existing;

    let tab = existing;
    if (tab?.kind === "script") {
      tab.script = script;
    } else {
      const session = createLogSession(scriptDriver(script.id), {
        onFinish: () => onScriptFinish(script.id),
      });
      tab = addTab({
        kind: "script",
        key: scriptKey(script.id),
        script,
        session,
      });
    }
    const launched = existing
      ? await tab.session.replay()
      : await tab.session.start();
    if (launched || tab.session.status !== "error") return tab;

    notify({
      tone: "error",
      title: "Script non lancé",
      detail: script.name,
      message: tab.session.error ?? undefined,
    });
    if (!existing) closeTab(tab.id);
    return existing;
  }

  /** Exécutions de scripts en cours. */
  const runningScripts = computed(() =>
    tabs.value.filter((tab) => tab.kind === "script" && tab.session.active),
  );

  // --- Commandes compose -----------------------------------------------------

  /** Onglet d'une commande compose (verbe et projet). */
  function composeTab(
    project: string,
    action: ComposeAction,
  ): (TelemetryTab & ComposeStream) | null {
    const tab = tabByKey(composeKey(project, action));
    return tab?.kind === "compose" ? tab : null;
  }

  /** Commandes compose en cours. */
  const runningCompose = computed(() =>
    tabs.value.filter(
      (tab): tab is TelemetryTab & ComposeStream =>
        tab.kind === "compose" && tab.session.active,
    ),
  );

  /** Commande compose en cours sur un projet : une seule à la fois. */
  function composeRunning(
    project: string,
  ): (TelemetryTab & ComposeStream) | null {
    return runningCompose.value.find((tab) => tab.project === project) ?? null;
  }

  function composeDriver(project: string, action: ComposeAction): StreamDriver {
    return {
      open: async (onEvent) => {
        const run = await composeApi.runCompose(project, action, onEvent);
        const tab = composeTab(project, action);
        if (tab) tab.command = run.command;
        return run.runId;
      },
      close: composeApi.stopCompose,
    };
  }

  /** Au-delà de la limite, les onglets compose terminés les plus anciens sont fermés. */
  function trimComposeTabs(keepId: string) {
    const composeTabs = tabs.value.filter((tab) => tab.kind === "compose");
    let excess = composeTabs.length - MAX_COMPOSE_TABS;
    for (const tab of composeTabs) {
      if (excess <= 0) break;
      if (tab.id === keepId || tab.session.active) continue;
      closeTab(tab.id);
      excess--;
    }
  }

  /**
   * Lance une commande compose (la confirmation est faite avant) dans son
   * onglet : relancer la même commande remplace son onglet terminé. Une
   * commande déjà en cours sur le projet est renvoyée telle quelle. Échec du
   * lancement (limite de flux, fichiers refusés, liaison absente) :
   * notification, et l'onglet créé pour l'occasion est retiré.
   */
  async function runCompose(
    project: string,
    action: ComposeAction,
    directory: string | null,
    onFinish: (project: string, action: ComposeAction) => void,
  ): Promise<TelemetryTab | null> {
    const running = composeRunning(project);
    if (running) return running;

    let tab = composeTab(project, action);
    const existing = tab;
    if (tab) {
      tab.directory = directory;
      tab.command = null;
    } else {
      const session = createLogSession(composeDriver(project, action), {
        onFinish: () => onFinish(project, action),
      });
      const added = addTab({
        kind: "compose",
        key: composeKey(project, action),
        project,
        action,
        command: null,
        directory,
        session,
      });
      trimComposeTabs(added.id);
      tab = composeTab(project, action);
    }
    if (!tab) return null;
    const launched = existing
      ? await tab.session.replay()
      : await tab.session.start();
    if (launched || tab.session.status !== "error") return tab;

    notify({
      tone: "error",
      title: "Commande non lancée",
      detail: `docker compose ${action} · ${project}`,
      message: tab.session.error ?? undefined,
    });
    if (!existing) closeTab(tab.id);
    return existing;
  }

  /** Fin de session (déconnexion voulue) : tout est vidé. */
  function reset() {
    closeDock();
    for (const tab of tabs.value) void tab.session.stop();
    tabs.value = [];
  }

  /** LOS : chaque flux en cours est marqué interrompu, rien n'est relancé. */
  function interruptAll() {
    dock.value?.session.interrupt();
    for (const tab of tabs.value) tab.session.interrupt();
  }

  connection.on("lost", interruptAll);
  connection.on("ended", reset);
  // La dernière exécution des scripts interrompus a été enregistrée.
  connection.on("restored", () => void scripts.load().catch(() => undefined));

  const tabCount = computed(() => tabs.value.length);

  return {
    dock,
    tabs,
    tabCount,
    tabById,
    scriptTab,
    runScript,
    runningScripts,
    composeTab,
    composeRunning,
    runningCompose,
    runCompose,
    openDock,
    closeDock,
    suspendDock,
    resumeDock,
    promoteDock,
    showLogs,
    closeTab,
  };
});
