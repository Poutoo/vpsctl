import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../api/presence";
import type { PresenceStatus } from "../bindings/PresenceStatus";
import { useNotify } from "../composables/useNotify";
import { lastToast } from "../testUtils";
import { usePreferencesStore } from "./preferences";
import { usePresenceStore } from "./presence";

vi.mock("../api/presence");
vi.mock("../api/preferences");

const phase = { phase: "ground" as const, since: 1_000 };
const startedAt = 500;
let emit: (status: PresenceStatus) => void = () => undefined;

beforeEach(() => {
  setActivePinia(createPinia());
  useNotify().items.splice(0);
  vi.mocked(api.presenceStatus).mockResolvedValue({
    state: "off",
    phase,
    startedAt,
  });
  vi.mocked(api.onPresenceStatus).mockImplementation(async (handler) => {
    emit = handler;
    return () => undefined;
  });
  vi.mocked(api.setPresenceEnabled).mockResolvedValue();
  vi.mocked(api.retryPresence).mockResolvedValue();
});

describe("présence Discord", () => {
  it("suit le statut publié par le backend", async () => {
    const presence = usePresenceStore();
    await presence.initialize();
    expect(presence.status?.state).toBe("off");
    expect(presence.available).toBe(true);
    emit({ state: "connected", phase, startedAt });
    expect(presence.status?.state).toBe("connected");
  });

  it("n'écoute qu'une fois", async () => {
    const presence = usePresenceStore();
    await Promise.all([presence.initialize(), presence.initialize()]);
    expect(api.onPresenceStatus).toHaveBeenCalledTimes(1);
  });

  it("est indisponible en développement ou sans identifiant", async () => {
    const presence = usePresenceStore();
    await presence.initialize();
    emit({ state: "devUnavailable", phase, startedAt });
    expect(presence.available).toBe(false);
    emit({ state: "notConfigured", phase, startedAt });
    expect(presence.available).toBe(false);
  });

  it("enregistre l'interrupteur dans les préférences", async () => {
    const presence = usePresenceStore();
    await presence.setEnabled(true);
    expect(api.setPresenceEnabled).toHaveBeenCalledWith(true);
    expect(usePreferencesStore().discordPresence).toBe(true);
    expect(presence.enabled).toBe(true);
  });

  it("revient en arrière si l'enregistrement échoue", async () => {
    vi.mocked(api.setPresenceEnabled).mockRejectedValue({
      kind: "Io",
      message: "disque plein",
    });
    const presence = usePresenceStore();
    await presence.setEnabled(true);
    expect(presence.enabled).toBe(false);
    expect(lastToast()?.tone).toBe("error");
  });

  it("« Réessayer » ne notifie jamais rien", async () => {
    vi.mocked(api.retryPresence).mockRejectedValue(new Error("absent"));
    const presence = usePresenceStore();
    presence.retry();
    await Promise.resolve();
    expect(useNotify().items).toHaveLength(0);
  });
});
