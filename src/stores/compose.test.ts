import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../api/compose";
import * as connectionApi from "../api/connection";
import { linked, lossEvent } from "../testUtils";
import { useComposeStore } from "./compose";

vi.mock("../api/compose");
vi.mock("../api/connection");

beforeEach(() => {
  setActivePinia(createPinia());
  vi.mocked(connectionApi.disconnect).mockResolvedValue();
});

describe("vérification des fichiers compose", () => {
  it("range le résultat par projet", async () => {
    linked();
    vi.mocked(api.checkProjects).mockResolvedValue([
      { project: "app", status: { kind: "ok" } },
      { project: "web", status: { kind: "dirMissing" } },
    ]);
    const compose = useComposeStore();
    await compose.check();
    expect(compose.checks).toEqual({
      app: { kind: "ok" },
      web: { kind: "dirMissing" },
    });
  });

  it("ne lance qu'une vérification à la fois", async () => {
    linked();
    vi.mocked(api.checkProjects).mockResolvedValue([]);
    const compose = useComposeStore();
    await Promise.all([compose.check(), compose.check()]);
    expect(api.checkProjects).toHaveBeenCalledTimes(1);
  });

  it("garde la vérification précédente après un échec", async () => {
    linked();
    vi.mocked(api.checkProjects).mockResolvedValueOnce([
      { project: "app", status: { kind: "ok" } },
    ]);
    const compose = useComposeStore();
    await compose.check();
    vi.mocked(api.checkProjects).mockRejectedValueOnce({
      kind: "Network",
      message: "délai dépassé",
    });
    await compose.check();
    expect(compose.checks).toEqual({ app: { kind: "ok" } });
  });

  it("ignore une réponse arrivée après un LOS et ne vérifie pas pendant", async () => {
    const connection = linked();
    let answer: (value: []) => void = () => undefined;
    vi.mocked(api.checkProjects).mockReturnValueOnce(
      new Promise((resolve) => (answer = resolve)),
    );
    const compose = useComposeStore();
    const pending = compose.check();
    connection.linkLost(lossEvent());
    answer([]);
    await pending;
    expect(compose.checks).toEqual({});
    await compose.check();
    expect(api.checkProjects).toHaveBeenCalledTimes(1);
  });

  it("oublie tout à la fin de la session", async () => {
    const connection = linked();
    vi.mocked(api.checkProjects).mockResolvedValue([
      { project: "app", status: { kind: "ok" } },
    ]);
    const compose = useComposeStore();
    await compose.check();
    await connection.disconnect();
    expect(compose.checks).toEqual({});
  });
});
