import type { Action } from "../bindings/Action";
import type { Service } from "../bindings/Service";
import { call } from "./invoke";

// Services systemd utilisateur (docs/04-contrat-commandes.md).

/** Services dont le fichier d'unité est dans `~/.config/systemd/user/`. */
export function listServices(): Promise<Service[]> {
  return call("services_list");
}

/**
 * Démarre, arrête ou redémarre un service pilotable. Rend la main dès que la
 * tâche est mise en file (`--no-block`) : l'état réel est à relire ensuite.
 */
export function serviceAction(unit: string, action: Action): Promise<void> {
  return call("service_action", { unit, action });
}
