import type { Container } from "../../bindings/Container";
import type { Service } from "../../bindings/Service";
import type { Operation } from "../../stores/services";

/** Raison d'indisponibilité de chaque action, ou `null` si elle est possible. */
export type Availability<K extends Operation> = Record<K, string | null>;

export type ContainerOperation = Operation;
export type ServiceOperation = Exclude<Operation, "remove">;

const IN_TRANSITION = "en transition";
const RUNNING = "déjà en marche";
const STOPPED = "déjà arrêté";
const STOP_FIRST = "arrêtez-le avant de le supprimer";

/**
 * Actions possibles sur un conteneur selon son état Docker. `docker rm`
 * sans `-f` refuse un conteneur en marche ou en pause ; la reprise d'un
 * conteneur en pause (`unpause`) n'est pas prise en charge.
 */
export function containerAvailability(
  container: Container,
): Availability<ContainerOperation> {
  if (container.state === "transition") {
    return {
      start: IN_TRANSITION,
      stop: IN_TRANSITION,
      restart: IN_TRANSITION,
      remove: IN_TRANSITION,
    };
  }
  switch (container.dockerState) {
    case "running":
      return { start: RUNNING, stop: null, restart: null, remove: STOP_FIRST };
    case "paused":
      return {
        start: "en pause : la reprise n'est pas prise en charge",
        stop: null,
        restart: "en pause : arrêtez-le d'abord",
        remove: STOP_FIRST,
      };
    case "exited":
    case "created":
      return { start: null, stop: STOPPED, restart: null, remove: null };
    case "dead": {
      const dead = "conteneur mort : il ne peut qu'être supprimé";
      return { start: dead, stop: STOPPED, restart: dead, remove: null };
    }
    default: {
      const unknown = `état ${container.dockerState} non pris en charge`;
      return {
        start: unknown,
        stop: unknown,
        restart: unknown,
        remove: unknown,
      };
    }
  }
}

/**
 * Actions possibles sur un service pilotable selon son état. Une unité
 * masquée ou non pilotable n'a que l'action Logs (aucun bouton d'action).
 */
export function serviceAvailability(
  service: Service,
): Availability<ServiceOperation> {
  switch (service.state) {
    case "transition":
      return {
        start: IN_TRANSITION,
        stop: IN_TRANSITION,
        restart: IN_TRANSITION,
      };
    case "nominal":
      return { start: RUNNING, stop: null, restart: null };
    case "stopped":
    case "failed":
      return { start: null, stop: STOPPED, restart: null };
  }
}

/** Libellé de l'état « en transition » pendant une action. */
export const OPERATION_PROGRESS: Record<Operation, string> = {
  start: "Démarrage",
  stop: "Arrêt",
  restart: "Redémarrage",
  remove: "Suppression",
};
