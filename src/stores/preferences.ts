import { computed, ref } from "vue";
import { defineStore } from "pinia";
import * as api from "../api/preferences";
import { errorMessage, toAppError } from "../api/errors";
import type { Preferences } from "../bindings/Preferences";
import { useNotify } from "../composables/useNotify";

/** Valeur du backend (`preferences::DEFAULT_DIMMING`), avant la lecture. */
export const DEFAULT_DIMMING = 85;
/** Délai d'enregistrement après le dernier mouvement du curseur. */
export const DIMMING_SAVE_DELAY_MS = 300;

/**
 * Préférences de l'interface. Les valeurs affichées changent tout de suite ;
 * l'enregistrement suit (après une courte pause pour le curseur). Un échec
 * d'enregistrement est notifié et la valeur reste celle choisie pour la
 * session.
 */
export const usePreferencesStore = defineStore("preferences", () => {
  const { notify } = useNotify();

  /** Préférences lues (ou lecture échouée : valeurs par défaut). */
  const ready = ref(false);
  const dimming = ref(DEFAULT_DIMMING);
  const originalColors = ref(false);

  /** Opacité de l'image de fond, déduite de l'assombrissement. */
  const backgroundOpacity = computed(() => (100 - dimming.value) / 100);

  let dimmingTimer: ReturnType<typeof setTimeout> | null = null;

  function apply(preferences: Preferences) {
    dimming.value = preferences.dimming;
    originalColors.value = preferences.originalColors;
  }

  function saveFailed(error: unknown) {
    notify({
      tone: "error",
      title: "Préférence non enregistrée",
      message: errorMessage(toAppError(error)),
    });
  }

  async function load() {
    try {
      apply(await api.getPreferences());
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

  return {
    ready,
    dimming,
    originalColors,
    backgroundOpacity,
    load,
    setDimming,
    setOriginalColors,
  };
});
