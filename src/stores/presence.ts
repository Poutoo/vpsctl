import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { errorMessage, toAppError } from "../api/errors";
import * as api from "../api/presence";
import type { PresenceStatus } from "../bindings/PresenceStatus";
import { useNotify } from "../composables/useNotify";
import { usePreferencesStore } from "./preferences";

/**
 * Présence Discord : statut tenu par le backend (événement
 * `presence-status`) et interrupteur, enregistré dans les préférences.
 * Discord absent n'est jamais notifié : le panneau l'affiche, avec
 * « Réessayer ».
 */
export const usePresenceStore = defineStore("presence", () => {
  const { notify } = useNotify();
  const preferences = usePreferencesStore();

  const status = ref<PresenceStatus | null>(null);
  const toggling = ref(false);
  let listening: Promise<void> | null = null;

  const enabled = computed(() => preferences.discordPresence);
  const available = computed(
    () =>
      status.value !== null &&
      status.value.state !== "devUnavailable" &&
      status.value.state !== "notConfigured",
  );

  /** Lit le statut et écoute ses changements, une seule fois. */
  function initialize(): Promise<void> {
    listening ??= (async () => {
      await api.onPresenceStatus((next) => (status.value = next));
      status.value = await api.presenceStatus();
    })().catch(() => {
      listening = null;
    });
    return listening;
  }

  async function setEnabled(value: boolean) {
    if (toggling.value) return;
    const previous = preferences.discordPresence;
    preferences.discordPresence = value;
    toggling.value = true;
    try {
      await api.setPresenceEnabled(value);
    } catch (error) {
      preferences.discordPresence = previous;
      notify({
        tone: "error",
        title: "Préférence non enregistrée",
        message: errorMessage(toAppError(error)),
      });
    } finally {
      toggling.value = false;
    }
  }

  function retry() {
    void api.retryPresence().catch(() => undefined);
  }

  return {
    status,
    toggling,
    enabled,
    available,
    initialize,
    setEnabled,
    retry,
  };
});
