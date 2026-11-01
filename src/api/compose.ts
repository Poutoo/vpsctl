import type { ProjectCheck } from "../bindings/ProjectCheck";
import { call } from "./invoke";

// Docker compose (docs/04-contrat-commandes.md).

/** Dossier et fichiers de chaque projet, vérifiés par SFTP. */
export function checkProjects(): Promise<ProjectCheck[]> {
  return call("compose_check");
}

/** Oublie un projet descendu du profil connecté ; rien n'est touché sur le serveur. */
export function forgetProject(project: string): Promise<void> {
  return call("compose_forget", { project });
}
