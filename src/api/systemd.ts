import type { Service } from "../bindings/Service";
import { call } from "./invoke";

// Services systemd utilisateur (docs/04-contrat-commandes.md).

/** Services dont le fichier d'unité est dans `~/.config/systemd/user/`. */
export function listServices(): Promise<Service[]> {
  return call("services_list");
}
