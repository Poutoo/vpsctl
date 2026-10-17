import { computed, ref, shallowRef, watch } from "vue";
import { defineStore } from "pinia";
import * as logs from "../api/logs";
import type { LogSource } from "../bindings/LogSource";
import {
  createLogSession,
  type LogSession,
  type StreamDriver,
} from "../composables/logSession";
import { router } from "../router";
import { useConnectionStore } from "./connection";

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

/** Flux affiché par une console : panneau ancré ou onglet. */
export type TelemetryStream = LogsStream;

/** Onglet de l'écran Télémétrie. */
export type TelemetryTab = TelemetryStream & {
  /** Identifiant local, dans l'URL (`/telemetry/:tab`). */
  id: string;
};

/** Section de la navigation dont vient un flux, surlignée sur l'écran Télémétrie. */
export function streamOrigin(stream: TelemetryStream): "dashboard" | "scripts" {
  return stream.kind === "logs" ? "dashboard" : "scripts";
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
    if (session && !session.active) void session.replay();
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

  /** Flux arrêtés par la déconnexion (ou une liaison perdue) : tout est vidé. */
  function reset() {
    closeDock();
    for (const tab of tabs.value) void tab.session.stop();
    tabs.value = [];
  }

  // Les flux appartiennent à la liaison.
  watch(
    () => connection.info?.connectedAt,
    () => reset(),
  );

  const tabCount = computed(() => tabs.value.length);

  return {
    dock,
    tabs,
    tabCount,
    tabById,
    openDock,
    closeDock,
    suspendDock,
    resumeDock,
    promoteDock,
    showLogs,
    closeTab,
  };
});
