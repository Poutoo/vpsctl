import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../api/preferences";
import { useNotify } from "../composables/useNotify";
import { lastToast } from "../testUtils";
import {
  DEFAULT_DIMMING,
  DIMMING_SAVE_DELAY_MS,
  usePreferencesStore,
} from "./preferences";

vi.mock("../api/preferences");

const saved = { dimming: 40, originalColors: true };

beforeEach(() => {
  setActivePinia(createPinia());
  useNotify().items.splice(0);
  vi.mocked(api.getPreferences).mockResolvedValue(saved);
  vi.mocked(api.setDimming).mockImplementation(async (dimming) => ({
    ...saved,
    dimming,
  }));
  vi.mocked(api.setOriginalColors).mockImplementation(async (enabled) => ({
    ...saved,
    originalColors: enabled,
  }));
});

afterEach(() => {
  vi.useRealTimers();
  vi.clearAllMocks();
});

describe("lecture", () => {
  it("n'est prête qu'après la lecture, avec les valeurs du backend", async () => {
    const preferences = usePreferencesStore();
    expect(preferences.ready).toBe(false);
    expect(preferences.dimming).toBe(DEFAULT_DIMMING);
    await preferences.load();
    expect(preferences.ready).toBe(true);
    expect(preferences.dimming).toBe(40);
    expect(preferences.originalColors).toBe(true);
    expect(preferences.backgroundOpacity).toBeCloseTo(0.6);
  });

  it("garde les valeurs par défaut si la lecture échoue", async () => {
    vi.mocked(api.getPreferences).mockRejectedValue({ kind: "Io" });
    const preferences = usePreferencesStore();
    await preferences.load();
    expect(preferences.ready).toBe(true);
    expect(preferences.dimming).toBe(DEFAULT_DIMMING);
    expect(preferences.backgroundOpacity).toBeCloseTo(0.15);
  });
});

describe("assombrissement", () => {
  it("s'applique tout de suite et n'enregistre que la dernière valeur", async () => {
    vi.useFakeTimers();
    const preferences = usePreferencesStore();
    preferences.setDimming(70);
    preferences.setDimming(64.6);
    expect(preferences.dimming).toBe(65);
    expect(api.setDimming).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(DIMMING_SAVE_DELAY_MS);
    expect(api.setDimming).toHaveBeenCalledExactlyOnceWith(65);
  });

  it("reste dans les bornes", () => {
    const preferences = usePreferencesStore();
    preferences.setDimming(130);
    expect(preferences.dimming).toBe(100);
    preferences.setDimming(-5);
    expect(preferences.dimming).toBe(0);
  });

  it("notifie un échec d'enregistrement sans revenir en arrière", async () => {
    vi.useFakeTimers();
    vi.mocked(api.setDimming).mockRejectedValue({
      kind: "Io",
      message: "disque plein",
    });
    const preferences = usePreferencesStore();
    preferences.setDimming(20);
    await vi.advanceTimersByTimeAsync(DIMMING_SAVE_DELAY_MS);
    expect(preferences.dimming).toBe(20);
    expect(lastToast()).toMatchObject({
      tone: "error",
      title: "Préférence non enregistrée",
    });
  });
});

describe("couleurs d'origine", () => {
  it("s'appliquent et sont enregistrées", async () => {
    const preferences = usePreferencesStore();
    await preferences.setOriginalColors(true);
    expect(preferences.originalColors).toBe(true);
    expect(api.setOriginalColors).toHaveBeenCalledWith(true);
  });
});
