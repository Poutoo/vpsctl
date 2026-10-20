import { useNotify } from "./composables/useNotify";

/** Dernière notification affichée (tests uniquement). */
export function lastToast() {
  const { items } = useNotify();
  return items[items.length - 1];
}
