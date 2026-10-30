import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../api/preferences";
import type { BackgroundInfo } from "../bindings/BackgroundInfo";
import { useNotify } from "../composables/useNotify";
import * as imageUrl from "../imageUrl";
import { lastToast } from "../testUtils";
import {
  BACKGROUND_LOST_MESSAGE,
  DEFAULT_DIMMING,
  DIMMING_SAVE_DELAY_MS,
  usePreferencesStore,
} from "./preferences";

vi.mock("../api/preferences");
vi.mock("../imageUrl");

const saved = { dimming: 40, originalColors: true, background: null };
const image: BackgroundInfo = {
  fileName: "nebuleuse-nord.jpg",
  format: "jpeg",
  bytes: 2_516_582,
  width: 3840,
  height: 2160,
};
const bytes = new ArrayBuffer(8);
let urls = 0;

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
  vi.mocked(api.backgroundLostAtStartup).mockResolvedValue(false);
  vi.mocked(api.backgroundImage).mockResolvedValue(bytes);
  vi.mocked(api.pendingBackgroundImage).mockResolvedValue(bytes);
  vi.mocked(api.commitBackground).mockResolvedValue({
    ...saved,
    background: image,
  });
  vi.mocked(api.resetBackground).mockResolvedValue(saved);
  vi.mocked(api.discardBackground).mockResolvedValue();
  vi.mocked(imageUrl.createImageUrl).mockImplementation(
    () => `blob:atlas/${++urls}`,
  );
  vi.mocked(imageUrl.decodeImage).mockResolvedValue();
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

describe("image de fond", () => {
  const pending = {
    kind: "pending" as const,
    token: "t1",
    background: image,
  };

  it("affiche la copie enregistrée une fois décodée", async () => {
    vi.mocked(api.getPreferences).mockResolvedValue({
      ...saved,
      background: image,
    });
    const preferences = usePreferencesStore();
    await preferences.load();
    expect(imageUrl.createImageUrl).toHaveBeenCalledWith(bytes, "jpeg");
    expect(preferences.backgroundUrl).toMatch(/^blob:/);
    expect(preferences.background).toEqual(image);
  });

  it("revient au fond d'origine si la copie ne se décode pas", async () => {
    vi.mocked(api.getPreferences).mockResolvedValue({
      ...saved,
      background: image,
    });
    vi.mocked(imageUrl.decodeImage).mockRejectedValue(new Error("illisible"));
    const preferences = usePreferencesStore();
    await preferences.load();
    expect(api.resetBackground).toHaveBeenCalled();
    expect(preferences.background).toBeNull();
    expect(preferences.backgroundUrl).toBeNull();
    expect(preferences.ready).toBe(true);
    expect(lastToast()).toMatchObject({
      tone: "error",
      message: BACKGROUND_LOST_MESSAGE,
    });
  });

  it("signale une copie écartée par le backend au démarrage", async () => {
    vi.mocked(api.backgroundLostAtStartup).mockResolvedValue(true);
    const preferences = usePreferencesStore();
    await preferences.load();
    expect(lastToast()?.message).toBe(BACKGROUND_LOST_MESSAGE);
  });

  it("enregistre une image décodée et l'affiche", async () => {
    vi.mocked(api.chooseBackground).mockResolvedValue(pending);
    const preferences = usePreferencesStore();
    await preferences.chooseBackground();
    expect(api.commitBackground).toHaveBeenCalledWith("t1");
    expect(preferences.background).toEqual(image);
    expect(preferences.backgroundUrl).toMatch(/^blob:/);
    expect(preferences.choosingSince).toBeNull();
  });

  it("garde le fond actuel et le dimming en cours après l'import", async () => {
    vi.mocked(api.chooseBackground).mockResolvedValue(pending);
    const preferences = usePreferencesStore();
    preferences.setDimming(10);
    await preferences.chooseBackground();
    expect(preferences.dimming).toBe(10);
  });

  it("refuse une image que le moteur web ne décode pas", async () => {
    vi.mocked(api.chooseBackground).mockResolvedValue(pending);
    vi.mocked(imageUrl.decodeImage).mockRejectedValue(new Error("illisible"));
    const preferences = usePreferencesStore();
    await preferences.chooseBackground();
    expect(api.discardBackground).toHaveBeenCalledWith("t1");
    expect(api.commitBackground).not.toHaveBeenCalled();
    expect(imageUrl.revokeImageUrl).toHaveBeenCalled();
    expect(preferences.background).toBeNull();
    expect(preferences.refusal).toEqual({
      fileName: "nebuleuse-nord.jpg",
      refusal: { kind: "unreadable" },
    });
  });

  it("affiche le refus du backend sans rien enregistrer", async () => {
    vi.mocked(api.chooseBackground).mockResolvedValue({
      kind: "refused",
      fileName: "capture-ecran.gif",
      refusal: { kind: "format" },
    });
    const preferences = usePreferencesStore();
    await preferences.chooseBackground();
    expect(preferences.refusal?.refusal.kind).toBe("format");
    expect(api.pendingBackgroundImage).not.toHaveBeenCalled();
  });

  it("ne change rien si la fenêtre est fermée sans choix", async () => {
    vi.mocked(api.chooseBackground).mockResolvedValue({ kind: "cancelled" });
    const preferences = usePreferencesStore();
    await preferences.chooseBackground();
    expect(preferences.refusal).toBeNull();
    expect(api.commitBackground).not.toHaveBeenCalled();
  });

  it("rétablit le fond d'origine", async () => {
    vi.mocked(api.chooseBackground).mockResolvedValue(pending);
    const preferences = usePreferencesStore();
    await preferences.chooseBackground();
    const url = preferences.backgroundUrl;
    await preferences.resetBackground();
    expect(preferences.background).toBeNull();
    expect(preferences.backgroundUrl).toBeNull();
    expect(imageUrl.revokeImageUrl).toHaveBeenCalledWith(url);
  });
});
