import { computed, ref } from "vue";
import { defineStore } from "pinia";
import * as api from "../api/preferences";
import { errorMessage, toAppError } from "../api/errors";
import type { BackgroundInfo } from "../bindings/BackgroundInfo";
import type { Preferences } from "../bindings/Preferences";
import type { Refusal } from "../bindings/Refusal";
import { useNotify } from "../composables/useNotify";
import { createImageUrl, decodeImage, revokeImageUrl } from "../imageUrl";

/** Valeur du backend (`preferences::DEFAULT_DIMMING`), avant la lecture. */
export const DEFAULT_DIMMING = 85;
/** Délai d'enregistrement après le dernier mouvement du curseur. */
export const DIMMING_SAVE_DELAY_MS = 300;

/** Message affiché quand la copie de l'image n'a pas pu être reprise. */
export const BACKGROUND_LOST_MESSAGE =
  "L'image enregistrée par Atlas est introuvable ou illisible. Atlas a repris le fond d'origine.";

/** Dernière image refusée, affichée jusqu'au choix suivant. */
export interface BackgroundRefusal {
  fileName: string;
  refusal: Refusal;
}

/**
 * Préférences de l'interface. Les valeurs affichées changent tout de suite ;
 * l'enregistrement suit (après une courte pause pour le curseur). Un échec
 * d'enregistrement est notifié et la valeur reste celle choisie pour la
 * session.
 *
 * Image de fond personnalisée : ses octets sont lus auprès du backend et
 * affichés par une URL `blob:`. Une image n'est enregistrée qu'après son
 * décodage réel par le moteur web ; au lancement, une copie que le moteur
 * ne sait pas décoder est abandonnée au profit du fond d'origine.
 */
export const usePreferencesStore = defineStore("preferences", () => {
  const { notify } = useNotify();

  /** Préférences lues (ou lecture échouée : valeurs par défaut). */
  const ready = ref(false);
  const dimming = ref(DEFAULT_DIMMING);
  const originalColors = ref(false);
  /** Interrupteur de la présence Discord (piloté par le store `presence`). */
  const discordPresence = ref(false);
  const background = ref<BackgroundInfo | null>(null);
  /** URL `blob:` de l'image personnalisée affichée. */
  const backgroundUrl = ref<string | null>(null);
  const refusal = ref<BackgroundRefusal | null>(null);
  /** Choix d'une image en cours, depuis (`performance.now()`). */
  const choosingSince = ref<number | null>(null);
  const resetting = ref(false);

  /** Opacité de l'image de fond, déduite de l'assombrissement. */
  const backgroundOpacity = computed(() => (100 - dimming.value) / 100);

  let dimmingTimer: ReturnType<typeof setTimeout> | null = null;

  function apply(preferences: Preferences) {
    dimming.value = preferences.dimming;
    originalColors.value = preferences.originalColors;
    discordPresence.value = preferences.discordPresence;
    background.value = preferences.background;
  }

  function showImage(url: string | null) {
    revokeImageUrl(backgroundUrl.value);
    backgroundUrl.value = url;
  }

  function notifyLost() {
    notify({
      tone: "error",
      title: "Image de fond abandonnée",
      message: BACKGROUND_LOST_MESSAGE,
    });
  }

  function failed(title: string, error: unknown) {
    notify({ tone: "error", title, message: errorMessage(toAppError(error)) });
  }

  /** Affiche la copie enregistrée, ou revient au fond d'origine. */
  async function showSavedImage(info: BackgroundInfo) {
    let url: string | null = null;
    try {
      url = createImageUrl(await api.backgroundImage(), info.format);
      await decodeImage(url);
      showImage(url);
    } catch {
      revokeImageUrl(url);
      try {
        background.value = (await api.resetBackground()).background;
      } catch {
        // Le fond d'origine s'affiche quand même pour cette session.
      }
      background.value = null;
      notifyLost();
    }
  }

  function saveFailed(error: unknown) {
    failed("Préférence non enregistrée", error);
  }

  async function load() {
    try {
      apply(await api.getPreferences());
      if (await api.backgroundLostAtStartup()) notifyLost();
      if (background.value) await showSavedImage(background.value);
    } catch {
      // Le backend redonne déjà les valeurs par défaut pour un fichier
      // illisible ; un échec ici laisse simplement celles de l'interface.
    } finally {
      ready.value = true;
    }
  }

  async function saveDimming() {
    dimmingTimer = null;
    try {
      await api.setDimming(dimming.value);
    } catch (error) {
      saveFailed(error);
    }
  }

  /** Suit le curseur ; l'enregistrement part après la dernière valeur. */
  function setDimming(value: number) {
    dimming.value = Math.min(100, Math.max(0, Math.round(value)));
    if (dimmingTimer !== null) clearTimeout(dimmingTimer);
    dimmingTimer = setTimeout(() => void saveDimming(), DIMMING_SAVE_DELAY_MS);
  }

  async function setOriginalColors(enabled: boolean) {
    originalColors.value = enabled;
    try {
      await api.setOriginalColors(enabled);
    } catch (error) {
      saveFailed(error);
    }
  }

  /**
   * Choix d'une image : refus affiché dans le panneau (format, taille,
   * dimensions, décodage) ; le fond actuel ne change qu'une fois la nouvelle
   * image enregistrée.
   */
  async function chooseBackground() {
    if (choosingSince.value !== null) return;
    choosingSince.value = performance.now();
    refusal.value = null;
    try {
      const choice = await api.chooseBackground();
      if (choice.kind === "cancelled") return;
      if (choice.kind === "refused") {
        refusal.value = { fileName: choice.fileName, refusal: choice.refusal };
        return;
      }
      const { token } = choice;
      const url = createImageUrl(
        await api.pendingBackgroundImage(token),
        choice.background.format,
      );
      try {
        await decodeImage(url);
      } catch {
        revokeImageUrl(url);
        await api.discardBackground(token).catch(() => undefined);
        refusal.value = {
          fileName: choice.background.fileName,
          refusal: { kind: "unreadable" },
        };
        return;
      }
      try {
        background.value = (await api.commitBackground(token)).background;
      } catch (error) {
        revokeImageUrl(url);
        throw error;
      }
      showImage(url);
    } catch (error) {
      failed("Image non enregistrée", error);
    } finally {
      choosingSince.value = null;
    }
  }

  async function resetBackground() {
    resetting.value = true;
    refusal.value = null;
    try {
      background.value = (await api.resetBackground()).background;
      showImage(null);
    } catch (error) {
      failed("Fond d'origine non rétabli", error);
    } finally {
      resetting.value = false;
    }
  }

  return {
    ready,
    dimming,
    originalColors,
    discordPresence,
    background,
    backgroundUrl,
    refusal,
    choosingSince,
    resetting,
    backgroundOpacity,
    load,
    setDimming,
    setOriginalColors,
    chooseBackground,
    resetBackground,
  };
});
