import { onBeforeUnmount, onMounted } from "vue";
import { watchWindowVisibility } from "../api/window";

/**
 * Exécute `task` au montage, puis `intervalMs` après la fin de chaque
 * exécution, tant que le composant est monté et la fenêtre visible (ni
 * masquée ni réduite, voir `watchWindowVisibility`). Au retour de la fenêtre,
 * la tâche est relancée immédiatement.
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
  /** Supposée visible au montage, en attendant l'état réel de la fenêtre. */
  let visible = true;
  let stopWatching: (() => void) | null = null;

  function clear() {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  }

  function schedule() {
    clear();
    if (mounted && visible) timer = setTimeout(trigger, intervalMs);
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

  function onVisibilityChange(next: boolean) {
    visible = next;
    if (visible) void trigger();
    else clear();
  }

  onMounted(() => {
    mounted = true;
    void trigger();
    void watchWindowVisibility(onVisibilityChange).then((watch) => {
      if (!mounted) {
        watch.stop();
        return;
      }
      stopWatching = watch.stop;
      if (!watch.visible) onVisibilityChange(false);
    });
  });

  onBeforeUnmount(() => {
    mounted = false;
    clear();
    stopWatching?.();
    stopWatching = null;
  });

  return { trigger };
}
