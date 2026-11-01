import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as composeApi from "../api/compose";
import * as docker from "../api/docker";
import type { ComposeAction } from "../bindings/ComposeAction";
import type { ComposeProject } from "../bindings/ComposeProject";
import type { LogEvent } from "../bindings/LogEvent";
import { useNotify } from "../composables/useNotify";
import { lastToast, linked, lossEvent } from "../testUtils";
import { useComposeStore } from "./compose";
import { MAX_COMPOSE_TABS, useTelemetryStore } from "./telemetry";

vi.mock("../api/compose");
vi.mock("../api/docker");
vi.mock("../api/systemd");
vi.mock("../api/logs");
vi.mock("../api/scripts");
vi.mock("../api/connection");

let events: ((event: LogEvent) => void)[];

beforeEach(() => {
  setActivePinia(createPinia());
  useNotify().items.splice(0);
  events = [];
  vi.mocked(composeApi.runCompose).mockImplementation(
    async (project, action: ComposeAction, onEvent) => {
      events.push(onEvent);
      return {
        runId: `run-${events.length}`,
        command: `docker compose --ansi never --progress plain -p '${project}' ${action}`,
      };
    },
  );
  vi.mocked(composeApi.stopCompose).mockResolvedValue();
  vi.mocked(composeApi.checkProjects).mockResolvedValue([]);
  vi.mocked(docker.listContainers).mockResolvedValue({
    containers: [],
    skipped: 0,
    projects: [],
  });
});

function project(name = "app"): ComposeProject {
  return {
    name,
    files: {
      workingDir: `/srv/${name}`,
      configFiles: [`/srv/${name}/compose.yaml`],
      envFiles: [],
    },
    problem: null,
    state: "nominal",
    running: 1,
    total: 1,
  };
}

describe("onglets des commandes compose", () => {
  it("ouvre un onglet verbe · projet avec la commande exacte", async () => {
    linked();
    const tab = await useComposeStore().run(project(), "up");
    expect(tab).toMatchObject({
      kind: "compose",
      project: "app",
      action: "up",
      directory: "/srv/app",
      command: "docker compose --ansi never --progress plain -p 'app' up",
    });
    expect(composeApi.runCompose).toHaveBeenCalledWith(
      "app",
      "up",
      expect.any(Function),
    );
  });

  it("passe par le nom seul pour Redémarrer et Arrêter", async () => {
    linked();
    const tab = await useComposeStore().run(project(), "down");
    expect(tab).toMatchObject({ directory: null });
  });

  it("ne lance qu'une commande à la fois par projet", async () => {
    linked();
    const compose = useComposeStore();
    const first = await compose.run(project(), "up");
    const second = await compose.run(project(), "pull");
    expect(second?.id).toBe(first?.id);
    expect(composeApi.runCompose).toHaveBeenCalledOnce();
    expect(compose.running("app")).toMatchObject({ action: "up" });
  });

  it("remplace l'onglet terminé de la même commande", async () => {
    linked();
    const compose = useComposeStore();
    const telemetry = useTelemetryStore();
    const first = await compose.run(project(), "pull");
    events[0]({ type: "ended", exitCode: 0 });
    const second = await compose.run(project(), "pull");
    expect(second?.id).toBe(first?.id);
    expect(telemetry.tabs).toHaveLength(1);
    expect(composeApi.runCompose).toHaveBeenCalledTimes(2);
  });

  it(`garde ${MAX_COMPOSE_TABS} onglets au plus, le plus ancien terminé sort`, async () => {
    linked();
    const compose = useComposeStore();
    const telemetry = useTelemetryStore();
    for (let i = 0; i < MAX_COMPOSE_TABS; i++) {
      await compose.run(project(`p${i}`), "pull");
      if (i !== 0) events[i]({ type: "ended", exitCode: 0 });
    }
    // p0 tourne encore : c'est p1, le plus ancien terminé, qui sort.
    await compose.run(project("p6"), "pull");
    const names = telemetry.tabs.map((tab) =>
      tab.kind === "compose" ? tab.project : "",
    );
    expect(names).toEqual(["p0", "p2", "p3", "p4", "p5", "p6"]);
  });

  it("retire l'onglet d'une commande refusée et le notifie", async () => {
    linked();
    vi.mocked(composeApi.runCompose).mockRejectedValueOnce({
      kind: "InvalidInput",
      message: "dossier introuvable : /srv/app",
    });
    const telemetry = useTelemetryStore();
    expect(await useComposeStore().run(project(), "up")).toBeNull();
    expect(telemetry.tabs).toHaveLength(0);
    expect(lastToast()).toMatchObject({
      title: "Commande non lancée",
      message: "dossier introuvable : /srv/app",
    });
  });
});

describe("issue d'une commande compose", () => {
  it("juge par le seul code de sortie et relit l'état", async () => {
    linked();
    const compose = useComposeStore();
    await compose.run(project(), "up");
    events[0]({
      type: "lines",
      lines: [
        {
          text: "Container app-web-1 Started",
          timestamp: null,
          origin: "stderr",
          priority: null,
          missing: false,
        },
      ],
      dropped: 0,
    });
    events[0]({ type: "ended", exitCode: 0 });
    await flushPromises();
    expect(lastToast()).toMatchObject({
      tone: "success",
      title: "Projet démarré",
    });
    expect(compose.outcome("app")).toBeNull();
    expect(docker.listContainers).toHaveBeenCalled();
    expect(composeApi.checkProjects).toHaveBeenCalled();
  });

  it("garde un échec sur le projet jusqu'à la commande suivante", async () => {
    linked();
    const compose = useComposeStore();
    const tab = await compose.run(project(), "pull");
    events[0]({ type: "ended", exitCode: 1 });
    expect(lastToast()).toMatchObject({
      tone: "error",
      title: "Échec de docker compose pull",
      action: { label: "Voir la sortie" },
    });
    expect(compose.outcome("app")).toEqual({
      action: "pull",
      text: "exit 1",
      tabId: tab?.id,
    });
    await compose.run(project(), "up");
    expect(compose.outcome("app")).toBeNull();
  });

  it("note une interruption demandée", async () => {
    linked();
    const compose = useComposeStore();
    const telemetry = useTelemetryStore();
    await compose.run(project(), "up");
    await flushPromises();
    const tab = telemetry.composeTab("app", "up");
    await compose.stop(tab!);
    expect(composeApi.stopCompose).toHaveBeenCalledWith("run-1");
    expect(compose.outcome("app")).toMatchObject({ text: "interrompue" });
  });

  it("une liaison perdue laisse l'issue inconnue, sans relance", async () => {
    const connection = linked();
    const compose = useComposeStore();
    await compose.run(project(), "up");
    await flushPromises();
    connection.linkLost(lossEvent());
    expect(compose.running("app")).toBeNull();
    expect(compose.outcome("app")).toMatchObject({
      action: "up",
      text: "issue inconnue",
    });
    expect(composeApi.runCompose).toHaveBeenCalledOnce();
    expect(useNotify().items).toHaveLength(0);
  });
});
