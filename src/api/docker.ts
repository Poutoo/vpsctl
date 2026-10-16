import type { Action } from "../bindings/Action";
import type { ContainerList } from "../bindings/ContainerList";
import { call } from "./invoke";

// Docker (docs/04-contrat-commandes.md).

/**
 * Tous les conteneurs, y compris arrêtés, triés par nom, et le nombre de
 * lignes illisibles ignorées.
 */
export function listContainers(): Promise<ContainerList> {
  return call("docker_list");
}

/** Démarre, arrête ou redémarre un conteneur (ID complet). */
export function dockerAction(id: string, action: Action): Promise<void> {
  return call("docker_action", { id, action });
}

/** Supprime un conteneur arrêté ; ses volumes et son image sont conservés. */
export function removeContainer(id: string): Promise<void> {
  return call("docker_remove", { id });
}
