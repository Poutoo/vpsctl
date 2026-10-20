import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defineComponent } from "vue";
import * as logs from "../api/logs";
import * as scriptsApi from "../api/scripts";
import type { LogEvent } from "../bindings/LogEvent";
import type { Script } from "../bindings/Script";
import { useDockedStream } from "../composables/useDockedStream";
import { useNotify } from "../composables/useNotify";
import { lastToast, linked, lossEvent } from "../testUtils";
import {
  containerLogTarget,
  serviceLogTarget,
  useTelemetryStore,
} from "./telemetry";

vi.mock("../api/logs");
vi.mock("../api/scripts");
vi.mock("../api/connection");

const backup: Script = {
  id: "s1",
  name: "backup-db.sh",
  description: "",
  command: "bash ~/scripts/backup-db.sh",
  requireConfirmation: true,
  lastRun: null,
};

let scriptEvents: ((event: LogEvent) => void)[];

beforeEach(() => {
  setActivePinia(createPinia());
  useNotify().items.splice(0);
  let next = 0;
  vi.mocked(logs.startLogs).mockImplementation(async () => `log-${++next}`);
  vi.mocked(logs.stopLogs).mockResolvedValue();
  scriptEvents = [];
  vi.mocked(scriptsApi.runScript).mockImplementation(async (_id, onEvent) => {
    scriptEvents.push(onEvent);
    return `run-${scriptEvents.length}`;
  });
  vi.mocked(scriptsApi.stopScript).mockResolvedValue();
  vi.mocked(scriptsApi.listScripts).mockResolvedValue([backup]);
});

const caddy = containerLogTarget("c".repeat(64), "caddy");

describe("panneau ancré", () => {
  it("n'ouvre qu'un flux par source et remplace l'ancien", async () => {
    const telemetry = useTelemetryStore();
    telemetry.openDock(caddy);
    telemetry.openDock(caddy);
    await flushPromises();
    expect(logs.startLogs).toHaveBeenCalledOnce();

    telemetry.openDock(serviceLogTarget("bot.service"));
    await flushPromises();
    expect(logs.stopLogs).toHaveBeenCalledWith("log-1");
    expect(telemetry.dock?.target.name).toBe("bot.service");
  });

  it("s'arrête au démontage du tableau de bord et repart au retour", async () => {
    const telemetry = useTelemetryStore();
    telemetry.openDock(caddy);
    await flushPromises();
    const Host = defineComponent({
      setup: () => useDockedStream(),
      render: () => null,
    });

    const host = mount(Host);
    await flushPromises();
    host.unmount();
    await flushPromises();
    expect(telemetry.dock?.session.status).toBe("stopped");
    expect(logs.stopLogs).toHaveBeenCalledWith("log-1");

    mount(Host);
    await flushPromises();
    expect(telemetry.dock?.session.status).toBe("live");
    expect(logs.startLogs).toHaveBeenCalledTimes(2);
  });

  it("passe en onglet sans redémarrer le flux", async () => {
    const telemetry = useTelemetryStore();
    telemetry.openDock(caddy);
    await flushPromises();
    const tab = telemetry.promoteDock();
    expect(telemetry.dock).toBeNull();
    expect(tab?.session.status).toBe("live");
    expect(telemetry.tabs).toHaveLength(1);
    expect(logs.stopLogs).not.toHaveBeenCalled();
  });
});

describe("scripts", () => {
  it("réutilise l'onglet d'un script et notifie la fin", async () => {
    const telemetry = useTelemetryStore();
    const tab = await telemetry.runScript(backup);
    expect(tab?.session.status).toBe("live");
    expect(await telemetry.runScript(backup)).toStrictEqual(tab);
    expect(telemetry.runningScripts).toHaveLength(1);

    scriptEvents[0]({ type: "ended", exitCode: 0 });
    await flushPromises();
    expect(telemetry.runningScripts).toHaveLength(0);
    expect(lastToast()).toMatchObject({
      tone: "success",
      title: "Procédure terminée",
    });

    await telemetry.runScript(backup);
    expect(telemetry.tabs).toHaveLength(1);
    expect(scriptsApi.runScript).toHaveBeenCalledTimes(2);
  });

  it("retire l'onglet créé si le script ne se lance pas", async () => {
    vi.mocked(scriptsApi.runScript).mockRejectedValueOnce({
      kind: "InvalidInput",
      message: "4 flux déjà ouverts",
    });
    const telemetry = useTelemetryStore();
    expect(await telemetry.runScript(backup)).toBeNull();
    expect(telemetry.tabs).toHaveLength(0);
    expect(lastToast()).toMatchObject({
      title: "Script non lancé",
      message: "4 flux déjà ouverts",
    });
  });

  it("garde l'onglet d'un script en échec avec un lien vers la sortie", async () => {
    const telemetry = useTelemetryStore();
    await telemetry.runScript(backup);
    scriptEvents[0]({ type: "ended", exitCode: 3 });
    expect(lastToast()).toMatchObject({
      tone: "error",
      title: "Procédure en échec",
      action: { label: "Voir la sortie" },
    });
  });
});

describe("liaison", () => {
  it("vide tout à la fin de la session", async () => {
    const connection = linked();
    const telemetry = useTelemetryStore();
    telemetry.openDock(caddy);
    await telemetry.runScript(backup);
    await connection.disconnect();
    expect(telemetry.dock).toBeNull();
    expect(telemetry.tabs).toHaveLength(0);
    expect(scriptsApi.stopScript).toHaveBeenCalledWith("run-1");
  });

  it("marque les flux interrompus au LOS, sans arrêt ni notification", async () => {
    const connection = linked();
    const telemetry = useTelemetryStore();
    telemetry.openDock(caddy);
    const tab = await telemetry.runScript(backup);
    await flushPromises();

    connection.linkLost(lossEvent());
    expect(tab?.session.status).toBe("lost");
    expect(telemetry.dock?.session.status).toBe("lost");
    expect(telemetry.runningScripts).toHaveLength(0);
    expect(telemetry.tabs).toHaveLength(1);
    expect(scriptsApi.stopScript).not.toHaveBeenCalled();
    expect(logs.stopLogs).not.toHaveBeenCalled();
    expect(useNotify().items).toHaveLength(0);

    scriptEvents[0]({ type: "ended", exitCode: 0 });
    expect(tab?.session.status).toBe("lost");
  });

  it("ne relance pas le panneau ancré au retour sur le tableau de bord", async () => {
    const connection = linked();
    const telemetry = useTelemetryStore();
    telemetry.openDock(caddy);
    await flushPromises();
    connection.linkLost(lossEvent());
    telemetry.resumeDock();
    await flushPromises();
    expect(logs.startLogs).toHaveBeenCalledOnce();
  });

  it("« Reprendre » rouvre les logs sans historique, à la suite des lignes gardées", async () => {
    const connection = linked();
    const telemetry = useTelemetryStore();
    telemetry.openDock(caddy);
    await flushPromises();
    connection.linkLost(lossEvent());

    const session = telemetry.dock!.session;
    await session.resume();
    expect(vi.mocked(logs.startLogs).mock.calls[1][1]).toBe(0);
    expect(session.status).toBe("live");
    const notice = session.lines[session.lines.length - 1];
    expect(notice.notice).toBe(true);
    expect(notice.text).toMatch(/^Flux interrompu à .+, repris à .+/);
  });
});
