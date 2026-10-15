import { computed, onBeforeUnmount, ref, watch, type Ref } from "vue";

/** Rafraîchissement de l'affichage d'un chrono qui tourne. */
const TICK_MS = 50;

/**
 * Chrono `T+` : durée écoulée affichée à la place d'une animation en boucle.
 * `"s"` : `T+02` (boutons) ; `"ms"` : `T+01.844` (télémétrie, interrogation).
 */
export function formatElapsed(ms: number, precision: "s" | "ms"): string {
  const safe = Math.max(0, ms);
  if (precision === "s") {
    return `T+${String(Math.floor(safe / 1000)).padStart(2, "0")}`;
  }
  const [whole, fraction] = (safe / 1000).toFixed(3).split(".");
  return `T+${whole.padStart(2, "0")}.${fraction}`;
}

/**
 * Durée écoulée depuis `since` (horodatage `performance.now()` ou
 * `Date.now()`, selon `clock`), relue toutes les 50 ms tant que `since`
 * n'est pas nul. Le minuteur s'arrête au démontage.
 */
export function useElapsed(
  since: Ref<number | null>,
  clock: () => number = () => performance.now(),
) {
  const now = ref(clock());
  let timer: ReturnType<typeof setInterval> | null = null;

  function stop() {
    if (timer !== null) clearInterval(timer);
    timer = null;
  }

  watch(
    since,
    (value) => {
      stop();
      now.value = clock();
      if (value !== null) {
        timer = setInterval(() => (now.value = clock()), TICK_MS);
      }
    },
    { immediate: true },
  );

  onBeforeUnmount(stop);

  return computed(() =>
    since.value === null ? null : Math.max(0, now.value - since.value),
  );
}
