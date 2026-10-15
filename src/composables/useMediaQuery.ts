import { onBeforeUnmount, ref } from "vue";

/** Suit une media query CSS ; l'écoute s'arrête au démontage. */
export function useMediaQuery(query: string) {
  const list = window.matchMedia(query);
  const matches = ref(list.matches);
  const update = (event: MediaQueryListEvent) =>
    (matches.value = event.matches);
  list.addEventListener("change", update);
  onBeforeUnmount(() => list.removeEventListener("change", update));
  return matches;
}

/**
 * Fenêtre étroite : la navigation se réduit aux numéraux. Doit rester égal
 * au seuil des `@media` des composants (une variable CSS n'y est pas utilisable).
 */
export const NARROW_QUERY = "(max-width: 1100px)";
