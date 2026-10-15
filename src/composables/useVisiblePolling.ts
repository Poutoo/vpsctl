import { onBeforeUnmount, onMounted } from "vue";

/**
 * Exécute `task` au montage, puis `intervalMs` après la fin de chaque
 * exécution, tant que le composant est monté et la fenêtre visible. Au retour
 * de la fenêtre, la tâche est relancée immédiatement.
 *
 * Le délai part de la fin de la tâche (pas de `setInterval`) : une tâche
 * lente ne s'empile jamais. `task` doit elle-même ignorer un appel pendant
 * qu'elle tourne (rafraîchissement manuel simultané).
 *
 * Renvoie `trigger`, qui lance la tâche tout de suite et repart pour un délai
 * complet.
 */
export function useVisiblePolling(
  task: () => Promise<void>,
  intervalMs: number,
) {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let mounted = false;

  function visible() {
    return document.visibilityState === "visible";
  }

  function clear() {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  }

  function schedule() {
    clear();
    if (mounted && visible()) timer = setTimeout(trigger, intervalMs);
  }

  async function trigger() {
    clear();
    try {
      await task();
    } catch {
      // L'erreur est affichée par la tâche ; le rafraîchissement continue.
    }
    schedule();
  }

  function onVisibilityChange() {
    if (visible()) void trigger();
    else clear();
  }

  onMounted(() => {
    mounted = true;
    document.addEventListener("visibilitychange", onVisibilityChange);
    void trigger();
  });

  onBeforeUnmount(() => {
    mounted = false;
    clear();
    document.removeEventListener("visibilitychange", onVisibilityChange);
  });

  return { trigger };
}
