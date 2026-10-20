import type { Action } from "../bindings/Action";
import type { Service } from "../bindings/Service";
import type { ServiceDef } from "../bindings/ServiceDef";
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

/**
 * Texte exact du fichier d'unité qui serait écrit (aperçu du formulaire).
 * Un champ invalide donne `InvalidInput`, dont le message nomme le champ.
 */
export function previewUnit(def: ServiceDef): Promise<string> {
  return call("service_unit_preview", { def });
}

/**
 * Écrit le fichier d'unité, recharge systemd, active et démarre le service
 * (`--no-block` : le résultat du démarrage est à vérifier ensuite).
 */
export function createService(def: ServiceDef): Promise<Service> {
  return call("service_create", { def });
}

/**
 * Active ou désactive un service au démarrage (`enable` / `disable`, sans le
 * démarrer ni l'arrêter), puis renvoie le service relu.
 */
export function setServiceStartup(
  unit: string,
  enabled: boolean,
): Promise<Service> {
  return call("service_set_startup", { unit, enabled });
}

/** Lingering du compte ; `null` si son état n'a pas pu être lu. */
export function userLinger(): Promise<boolean | null> {
  return call("user_linger");
}
