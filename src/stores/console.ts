import { ref, watch } from "vue";
import { defineStore } from "pinia";
import type { LogSource } from "../bindings/LogSource";
import { useConnectionStore } from "./connection";

/** Source affichée dans la console ancrée du tableau de bord. */
export interface LogTarget {
  source: LogSource;
  kind: "docker" | "systemd";
  /** Nom du conteneur ou de l'unité. */
  name: string;
  /** Commande équivalente, affichée dans l'en-tête de la console. */
  command: string;
}

export function containerLogTarget(id: string, name: string): LogTarget {
  return {
    source: { docker: id },
    kind: "docker",
    name,
    command: `docker logs -f ${name}`,
  };
}

export function serviceLogTarget(unit: string): LogTarget {
  return {
    source: { systemd: unit },
    kind: "systemd",
    name: unit,
    command: `journalctl --user -fu ${unit}`,
  };
}

/**
 * Console ancrée : une seule source à la fois. Le flux lui-même est géré par
 * `useLogStream` dans le composant affiché ; fermer le panneau le démonte,
 * ce qui arrête le flux.
 */
export const useConsoleStore = defineStore("console", () => {
  const connection = useConnectionStore();
  const target = ref<LogTarget | null>(null);

  /** Ouvre la console sur une source ; déjà affichée, rien ne change. */
  function open(next: LogTarget) {
    const current = target.value;
    if (
      current &&
      JSON.stringify(current.source) === JSON.stringify(next.source)
    ) {
      return;
    }
    target.value = next;
  }

  function close() {
    target.value = null;
  }

  // Les flux appartiennent à la liaison.
  watch(
    () => connection.info?.connectedAt,
    () => close(),
  );

  return { target, open, close };
});
