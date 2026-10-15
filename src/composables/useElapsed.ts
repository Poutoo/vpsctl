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

/** Durée longue au format `02:14:07` (heures, minutes, secondes). */
export function formatClock(ms: number): string {
  const total = Math.floor(Math.max(0, ms) / 1000);
  return [Math.floor(total / 3600), Math.floor(total / 60) % 60, total % 60]
    .map((part) => String(part).padStart(2, "0"))
    .join(":");
}

/**
 * Durée écoulée depuis `since` (horodatage `performance.now()` par défaut,
 * ou celui de `clock`), relue toutes les `tickMs` tant que `since` n'est pas
 * nul. Le minuteur s'arrête au démontage.
 */
export function useElapsed(
  since: Ref<number | null>,
  {
    clock = () => performance.now(),
    tickMs = TICK_MS,
  }: { clock?: () => number; tickMs?: number } = {},
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
        timer = setInterval(() => (now.value = clock()), tickMs);
      }
    },
    { immediate: true },
  );

  onBeforeUnmount(stop);

  return computed(() =>
    since.value === null ? null : Math.max(0, now.value - since.value),
  );
}
