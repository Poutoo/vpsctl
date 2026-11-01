import { call } from "./invoke";

// Docker compose (docs/04-contrat-commandes.md).

/** Oublie un projet descendu du profil connecté ; rien n'est touché sur le serveur. */
export function forgetProject(project: string): Promise<void> {
  return call("compose_forget", { project });
}
