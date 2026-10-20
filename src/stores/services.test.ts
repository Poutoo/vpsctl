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
