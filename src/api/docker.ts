import type { Container } from "../bindings/Container";
import { call } from "./invoke";

// Docker (docs/04-contrat-commandes.md).

/** Tous les conteneurs, y compris arrêtés, triés par nom. */
export function listContainers(): Promise<Container[]> {
  return call("docker_list");
}
