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
