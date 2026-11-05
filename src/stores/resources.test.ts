import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as connectionApi from "../api/connection";
import * as api from "../api/resources";
import type { AnalysisEvent } from "../bindings/AnalysisEvent";
import { HISTORY_MS } from "../resourceChart";
import { DEFAULT_THRESHOLDS } from "../resourceRules";
import { linked, lossEvent, sampleUpdate } from "../testUtils";
import {
  HIGHLIGHT_MS,
  MAX_POINTS,
  pushPoint,
  useResourcesStore,
} from "./resources";

vi.mock("../api/resources");
vi.mock("../api/connection");

beforeEach(() => {
  setActivePinia(createPinia());
  vi.mocked(connectionApi.disconnect).mockResolvedValue();
  vi.mocked(api.watchResources).mockResolvedValue();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("historique", () => {
  it("ajoute un point par relevé, pas par réémission du même relevé", () => {
    linked();
    const store = useResourcesStore();
    store.receive(sampleUpdate({ at: 1_000 }));
    store.receive(sampleUpdate({ at: 6_000 }));
    // Seuils changés : même relevé, réémis.
    store.receive(
      sampleUpdate({
        at: 6_000,
        levels: { disks: [], memory: "nominal", load: "nominal" },
      }),
    );
    expect(store.series.cpu.map((p) => p.at)).toEqual([1_000, 6_000]);
    expect(store.series.memory).toHaveLength(2);
    expect(store.series.load5[0].value).toBe(0.675);
    expect(store.series.received[1].value).toBe(1.8e6);
    expect(store.update?.levels.memory).toBe("nominal");
  });

  it("ne garde que 30 minutes et 360 points au plus", () => {
    let points = [{ at: 0, value: 1 }];
    points = pushPoint(points, { at: HISTORY_MS + 1, value: 2 });
    expect(points).toEqual([{ at: HISTORY_MS + 1, value: 2 }]);
    let many: { at: number; value: number }[] = [];
    for (let i = 0; i < MAX_POINTS + 10; i++)
      many = pushPoint(many, { at: i, value: i });
    expect(many).toHaveLength(MAX_POINTS);
    expect(many[0].at).toBe(10);
  });

  it("une mesure non relevée n'ajoute pas de point", () => {
    linked();
    const store = useResourcesStore();
    store.receive(
      sampleUpdate({
        foreground: false,
        cpu: { status: "notRead" },
        load: { status: "notRead" },
        network: { status: "notRead" },
      }),
    );
    expect(store.series.cpu).toEqual([]);
    expect(store.series.memory).toHaveLength(1);
  });

  it("ignore le relevé d'un autre profil", () => {
    linked();
    const store = useResourcesStore();
    store.receive(sampleUpdate({ profileId: "p2" }));
    expect(store.update).toBeNull();
  });
});

describe("perte de liaison", () => {
  it("ouvre une zone sans relevé, fermée au premier relevé suivant", () => {
    const connection = linked();
    const store = useResourcesStore();
    store.receive(sampleUpdate({ at: 1_000 }));
    connection.linkLost(lossEvent());
    expect(store.gaps).toEqual([{ from: 1_000, to: null }]);
    expect(store.resumed).toBe(true);
    store.receive(sampleUpdate({ at: 60_000, cpu: { status: "pending" } }));
    expect(store.gaps).toEqual([{ from: 1_000, to: 60_000 }]);
    // CPU figé jusqu'au deuxième relevé.
    expect(store.resumed).toBe(true);
    store.receive(sampleUpdate({ at: 65_000 }));
    expect(store.resumed).toBe(false);
    expect(store.series.cpu.map((p) => p.at)).toEqual([1_000, 65_000]);
  });

  it("une analyse en cours est à relancer", async () => {
    const connection = linked();
    vi.mocked(api.analyzeDocker).mockResolvedValue("run-1");
    const store = useResourcesStore();
    await store.analyze("volumes");
    connection.linkLost(lossEvent());
    expect(store.analysis).toEqual({ status: "lost", kind: "volumes" });
  });

  it("la déconnexion vide tout", async () => {
    const connection = linked();
    const store = useResourcesStore();
    store.receive(sampleUpdate());
    await connection.disconnect();
    expect(store.update).toBeNull();
    expect(store.series.cpu).toEqual([]);
    expect(store.thresholds).toBeNull();
  });
});

describe("analyse Docker", () => {
  it("garde le résultat des volumes et revient au repos", async () => {
    linked();
    let emit: (event: AnalysisEvent) => void = () => undefined;
    vi.mocked(api.analyzeDocker).mockImplementation(async (_kind, onEvent) => {
      emit = onEvent;
      return "run-1";
    });
    const store = useResourcesStore();
    await store.analyze("volumes");
    expect(store.analysis).toMatchObject({ status: "running", runId: "run-1" });
    emit({
      type: "done",
      result: {
        kind: "volumes",
        volumes: { at: 5, durationMs: 192_000, volumes: [] },
      },
    });
    expect(store.analysis).toEqual({ status: "idle" });
    expect(store.volumes?.durationMs).toBe(192_000);
  });

  it("annuler arrête la commande et ignore sa fin", async () => {
    linked();
    let emit: (event: AnalysisEvent) => void = () => undefined;
    vi.mocked(api.analyzeDocker).mockImplementation(async (_kind, onEvent) => {
      emit = onEvent;
      return "run-2";
    });
    vi.mocked(api.stopAnalysis).mockResolvedValue();
    const store = useResourcesStore();
    await store.analyze("volumes");
    await store.cancelAnalysis();
    expect(api.stopAnalysis).toHaveBeenCalledWith("run-2");
    emit({ type: "failed", message: "interrompue" });
    expect(store.analysis).toEqual({ status: "idle" });
  });

  it("un refus (4 flux) est affiché", async () => {
    linked();
    vi.mocked(api.analyzeDocker).mockRejectedValue({
      kind: "InvalidInput",
      message:
        "4 flux déjà ouverts : fermez un onglet de télémétrie pour lancer l'analyse",
    });
    const store = useResourcesStore();
    await store.analyze("volumes");
    expect(store.analysis).toMatchObject({ status: "failed", kind: "volumes" });
  });
});

describe("seuils et écran", () => {
  it("enregistre les seuils du profil de la session, même pendant un LOS", async () => {
    const connection = linked();
    connection.linkLost(lossEvent());
    const custom = {
      ...DEFAULT_THRESHOLDS,
      disk: { attention: 50, critical: 60 },
    };
    vi.mocked(api.saveThresholds).mockResolvedValue(custom);
    const store = useResourcesStore();
    await store.saveThresholds(custom);
    expect(api.saveThresholds).toHaveBeenCalledWith("p1", custom);
    expect(store.thresholds).toEqual(custom);
  });

  it("signale l'écran au backend une seule fois par changement", async () => {
    linked();
    const store = useResourcesStore();
    await store.setOnScreen(true);
    await store.setOnScreen(true);
    await store.setOnScreen(false);
    expect(vi.mocked(api.watchResources).mock.calls).toEqual([[true], [false]]);
  });

  it("le repère compte la charge seulement à l'écran", async () => {
    linked();
    const store = useResourcesStore();
    store.receive(
      sampleUpdate({
        levels: { disks: [], memory: "nominal", load: "attention" },
      }),
    );
    expect(store.level).toBe("nominal");
    await store.setOnScreen(true);
    expect(store.level).toBe("attention");
  });

  it("surligne un conteneur quelques secondes", () => {
    vi.useFakeTimers();
    linked();
    const store = useResourcesStore();
    store.highlight("abc");
    expect(store.highlighted).toBe("abc");
    vi.advanceTimersByTime(HIGHLIGHT_MS);
    expect(store.highlighted).toBeNull();
  });
});
