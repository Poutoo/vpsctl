import type { Action } from "../bindings/Action";
import type { Service } from "../bindings/Service";
import type { ServiceDeleteInfo } from "../bindings/ServiceDeleteInfo";
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

/** Fichier et dossier de surcharge qu'une suppression retirerait (GO / NO GO). */
export function serviceDeleteInfo(unit: string): Promise<ServiceDeleteInfo> {
  return call("service_delete_info", { unit });
}

/**
 * Arrête, désactive et supprime un service (fichier et dossier de surcharge),
 * puis recharge systemd. Rend la main à la fin de la séquence ; un échec en
 * cours de route est un `ServiceStepFailed`.
 */
export function deleteService(unit: string): Promise<void> {
  return call("service_delete", { unit });
}
