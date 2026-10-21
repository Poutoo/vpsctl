import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as docker from "../api/docker";
import * as systemd from "../api/systemd";
import type { Container } from "../bindings/Container";
import type { RunState } from "../bindings/RunState";
import type { Service } from "../bindings/Service";
import { useNotify } from "../composables/useNotify";
import * as connectionApi from "../api/connection";
import { lastToast, linked, lossEvent } from "../testUtils";
import { serviceTarget, targetKey, useServicesStore } from "./services";

vi.mock("../api/docker");
vi.mock("../api/systemd");
vi.mock("../api/connection");
vi.mock("../api/logs");
vi.mock("../api/scripts");

function service(unit: string, state: RunState): Service {
  return {
    unit,
    load: "loaded",
    active: state === "nominal" ? "active" : "failed",
    sub: state === "nominal" ? "running" : "failed",
    description: null,
    state,
    masked: false,
    controllable: true,
    deleteBlock: null,
    startup: "disabled",
    startupBlock: null,
  };
}

function container(state: RunState): Container {
  return {
    id: "a".repeat(64),
    name: "caddy",
    image: "caddy:2.8",
    state,
    dockerState: state === "nominal" ? "running" : "exited",
    exitCode: state === "nominal" ? null : 1,
    status: "",
    duration: null,
    oomKilled: false,
    memoryLimit: null,
  };
}

let units: Service[];

beforeEach(() => {
  vi.useFakeTimers();
  setActivePinia(createPinia());
  useNotify().items.splice(0);
  units = [service("bot.service", "stopped")];
  vi.mocked(systemd.listServices).mockImplementation(async () => units);
  vi.mocked(docker.listContainers).mockResolvedValue({
    containers: [container("nominal")],
    skipped: 0,
  });
  vi.mocked(systemd.serviceAction).mockResolvedValue();
});

afterEach(() => {
  vi.useRealTimers();
});

const bot = serviceTarget(service("bot.service", "stopped"));

describe("rafraîchissement", () => {
  it("charge les deux listes indépendamment", async () => {
    vi.mocked(docker.listContainers).mockRejectedValueOnce({
      kind: "DockerNotInstalled",
    });
    const services = useServicesStore();
    await services.refresh();
    expect(services.containers.error).toEqual({ kind: "DockerNotInstalled" });
    expect(services.containers.loaded).toBe(true);
    expect(services.services.items).toHaveLength(1);
    expect(services.refreshedAt).not.toBeNull();
  });

  it("réutilise un rafraîchissement en cours", async () => {
    const services = useServicesStore();
    await Promise.all([services.refresh(), services.refresh()]);
    expect(systemd.listServices).toHaveBeenCalledOnce();
  });
});

describe("action puis vérification", () => {
  it("reste en transition jusqu'à la relecture à 2,5 s", async () => {
    const services = useServicesStore();
    await services.refresh();
    const done = services.perform(bot, "start");
    await flushPromises();
    expect(services.pending[targetKey(bot)]?.operation).toBe("start");

    units = [service("bot.service", "nominal")];
    await vi.advanceTimersByTimeAsync(2_499);
    expect(services.pending[targetKey(bot)]).toBeDefined();
    await vi.advanceTimersByTimeAsync(1);
    await done;

    expect(services.pending[targetKey(bot)]).toBeUndefined();
    expect(lastToast()).toMatchObject({
      tone: "success",
      title: "Service démarré",
    });
    expect(services.arrivals[targetKey(bot)]?.direction).toBe("on");
  });

  it("signale un démarrage qui échoue à la relecture", async () => {
    const services = useServicesStore();
    await services.refresh();
    units = [service("bot.service", "failed")];
    const done = services.perform(bot, "restart");
    await vi.advanceTimersByTimeAsync(2_500);
    await done;
    expect(lastToast()).toMatchObject({
      tone: "error",
      title: "Échec du redémarrage",
      detail: "bot.service · failed/failed",
    });
  });

  it("relit tout de suite après un arrêt Docker", async () => {
    vi.mocked(docker.dockerAction).mockResolvedValue();
    const services = useServicesStore();
    await services.refresh();
    const caddy = services.containers.items[0];
    vi.mocked(docker.listContainers).mockResolvedValue({
      containers: [container("stopped")],
      skipped: 0,
    });
    await services.perform(
      { kind: "docker", id: caddy.id, name: caddy.name },
      "stop",
    );
    expect(lastToast()).toMatchObject({
      title: "Conteneur arrêté",
    });
  });

  it("signale l'échec de la commande elle-même", async () => {
    vi.mocked(systemd.serviceAction).mockRejectedValueOnce({
      kind: "Remote",
      exitCode: 5,
      stderr: "unit not found",
    });
    const services = useServicesStore();
    await services.refresh();
    await services.perform(bot, "start");
    expect(services.pending[targetKey(bot)]).toBeUndefined();
    expect(lastToast()).toMatchObject({
      tone: "error",
      title: "Échec du démarrage",
    });
  });

  it("ignore une seconde action sur la même ligne", async () => {
    const services = useServicesStore();
    await services.refresh();
    void services.perform(bot, "start");
    void services.perform(bot, "stop");
    await flushPromises();
    expect(systemd.serviceAction).toHaveBeenCalledOnce();
  });
});

describe("liaison", () => {
  it("vide les listes et les vérifications à la fin de la session", async () => {
    const connection = linked();
    vi.mocked(connectionApi.disconnect).mockResolvedValue();
    const services = useServicesStore();
    await services.refresh();
    const done = services.perform(bot, "start");
    await flushPromises();

    await connection.disconnect();
    expect(services.services.items).toHaveLength(0);
    expect(services.pending).toEqual({});

    await vi.advanceTimersByTimeAsync(2_500);
    await done;
    expect(useNotify().items).toHaveLength(0);
  });

  it("annule la vérification à 2,5 s sans la présenter comme un échec", async () => {
    const connection = linked();
    const services = useServicesStore();
    await services.refresh();
    const done = services.perform(bot, "start");
    await flushPromises();

    connection.linkLost(lossEvent());
    expect(services.pending).toEqual({});
    await vi.advanceTimersByTimeAsync(2_500);
    await done;
    expect(useNotify().items).toHaveLength(0);
    expect(systemd.listServices).toHaveBeenCalledOnce();
  });

  it("n'affiche pas d'échec quand l'action est coupée par la perte", async () => {
    linked();
    vi.mocked(systemd.serviceAction).mockRejectedValueOnce({
      kind: "LinkLost",
    });
    const services = useServicesStore();
    await services.refresh();
    await services.perform(bot, "start");
    expect(services.pending).toEqual({});
    expect(useNotify().items).toHaveLength(0);
  });

  it("fige la dernière lecture réussie et n'interroge plus le serveur", async () => {
    const connection = linked();
    const services = useServicesStore();
    await services.refresh();
    const loadedAt = services.services.loadedAt;
    vi.mocked(systemd.listServices).mockRejectedValueOnce({
      kind: "Network",
      message: "le serveur n'a pas répondu en 10 s",
    });
    await services.refresh();
    expect(services.services.error).not.toBeNull();

    connection.linkLost(lossEvent());
    expect(services.services.error).toBeNull();
    expect(services.services.items).toHaveLength(1);
    expect(services.frozenAt).toBe(loadedAt);

    await services.refresh();
    expect(systemd.listServices).toHaveBeenCalledTimes(2);
  });

  it("relit les listes au rétablissement", async () => {
    const connection = linked();
    vi.mocked(connectionApi.connect).mockResolvedValue({
      profileId: "p1",
      connectedAt: 5,
      latencyMs: null,
    });
    const services = useServicesStore();
    await services.refresh();
    connection.linkLost(lossEvent());
    await connection.reacquire();
    await flushPromises();
    expect(systemd.listServices).toHaveBeenCalledTimes(2);
    expect(services.frozenAt).toBeNull();
  });
});

describe("filtre du tableau de bord", () => {
  it("filtre les deux listes avec le même texte", async () => {
    units = [
      { ...service("api-reseau.service", "nominal"), description: "API" },
      { ...service("bot.service", "stopped"), description: "Bot Réseau" },
      service("cron.service", "stopped"),
    ];
    const services = useServicesStore();
    await services.refresh();
    services.filter = "RESEAU";
    expect(services.shownServices.map((s) => s.unit)).toEqual([
      "api-reseau.service",
      "bot.service",
    ]);
    expect(services.shownContainers).toHaveLength(0);
    services.filter = "caddy:2";
    expect(services.shownContainers).toHaveLength(1);
  });

  it("garde le filtre sur les données figées pendant un LOS", async () => {
    const connection = linked();
    units = [
      service("bot.service", "stopped"),
      service("api.service", "nominal"),
    ];
    const services = useServicesStore();
    await services.refresh();
    services.filter = "bot";
    connection.linkLost(lossEvent());
    expect(services.filter).toBe("bot");
    expect(services.shownServices.map((s) => s.unit)).toEqual(["bot.service"]);
  });

  it("vide le filtre à la fin de la session", async () => {
    const connection = linked();
    vi.mocked(connectionApi.disconnect).mockResolvedValue();
    const services = useServicesStore();
    await services.refresh();
    services.filter = "bot";
    await connection.disconnect();
    expect(services.filter).toBe("");
  });
});

describe("activation au démarrage", () => {
  beforeEach(() => {
    vi.mocked(systemd.setServiceStartup).mockImplementation(
      async (unit, enabled) => {
        units = units.map((s) =>
          s.unit === unit
            ? { ...s, startup: enabled ? "enabled" : "disabled" }
            : s,
        );
        return units.find((s) => s.unit === unit)!;
      },
    );
  });

  it("montre l'état relu, sans notification de succès", async () => {
    const services = useServicesStore();
    await services.refresh();
    const done = services.setStartup("bot.service", true);
    expect(services.startupPending["bot.service"]).toMatchObject({
      enabled: true,
    });
    await done;
    expect(systemd.setServiceStartup).toHaveBeenCalledWith("bot.service", true);
    expect(services.services.items[0].startup).toBe("enabled");
    expect(services.startupPending).toEqual({});
    expect(useNotify().items).toHaveLength(0);
  });

  it("garde l'interrupteur en cours jusqu'à une relecture partie après la commande", async () => {
    const services = useServicesStore();
    await services.refresh();
    const before = vi.mocked(systemd.listServices).mock.calls.length;
    await services.setStartup("bot.service", true);
    expect(vi.mocked(systemd.listServices).mock.calls.length).toBe(before + 1);
  });

  it("ignore un second clic pendant le changement", async () => {
    const services = useServicesStore();
    await services.refresh();
    void services.setStartup("bot.service", true);
    void services.setStartup("bot.service", false);
    await flushPromises();
    expect(systemd.setServiceStartup).toHaveBeenCalledOnce();
  });

  it("notifie un échec et relit la liste", async () => {
    vi.mocked(systemd.setServiceStartup).mockRejectedValueOnce({
      kind: "Remote",
      exitCode: 1,
      stderr: "Failed to enable unit",
    });
    const services = useServicesStore();
    await services.refresh();
    await services.setStartup("bot.service", true);
    expect(lastToast().title).toBe("Échec de l'activation au démarrage");
    expect(lastToast().tone).toBe("error");
    expect(services.startupPending).toEqual({});
  });

  it("signale un état relu différent de celui demandé", async () => {
    vi.mocked(systemd.setServiceStartup).mockResolvedValueOnce({
      ...service("bot.service", "stopped"),
      startup: "static",
    });
    const services = useServicesStore();
    await services.refresh();
    await services.setStartup("bot.service", true);
    expect(lastToast().message).toContain("static");
  });

  it("ne présente pas une perte de liaison comme un échec", async () => {
    const connection = linked();
    let fail: (error: unknown) => void = () => {};
    vi.mocked(systemd.setServiceStartup).mockImplementationOnce(
      () => new Promise((_, reject) => (fail = reject)),
    );
    const services = useServicesStore();
    await services.refresh();
    const done = services.setStartup("bot.service", true);
    connection.linkLost(lossEvent());
    expect(services.startupPending).toEqual({});
    fail({ kind: "LinkLost" });
    await done;
    expect(useNotify().items).toHaveLength(0);
    expect(services.services.items[0].startup).toBe("disabled");
  });

  it("ne lance rien pendant un LOS", async () => {
    const connection = linked();
    const services = useServicesStore();
    await services.refresh();
    connection.linkLost(lossEvent());
    await services.setStartup("bot.service", true);
    expect(systemd.setServiceStartup).not.toHaveBeenCalled();
  });
});

describe("conteneur tué par manque de mémoire", () => {
  it("le signale dans l'échec d'un démarrage", async () => {
    vi.mocked(docker.dockerAction).mockResolvedValue();
    const services = useServicesStore();
    await services.refresh();
    vi.mocked(docker.listContainers).mockResolvedValue({
      containers: [
        {
          ...container("failed"),
          exitCode: 137,
          oomKilled: true,
          memoryLimit: 536_870_912,
        },
      ],
      skipped: 0,
    });
    const target = {
      kind: "docker" as const,
      id: "a".repeat(64),
      name: "caddy",
    };
    const done = services.perform(target, "start");
    await vi.advanceTimersByTimeAsync(2_500);
    await done;
    expect(lastToast().detail).toBe("caddy · code 137 · manque de mémoire");
  });
});
