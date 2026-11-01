import { Channel } from "@tauri-apps/api/core";
import type { ComposeAction } from "../bindings/ComposeAction";
import type { ComposeRun } from "../bindings/ComposeRun";
import type { DownInfo } from "../bindings/DownInfo";
import type { LogEvent } from "../bindings/LogEvent";
import type { ProjectCheck } from "../bindings/ProjectCheck";
import { call } from "./invoke";

// Docker compose (docs/04-contrat-commandes.md).

/**
 * Lance une commande compose sur un projet relu sur le serveur ; sa sortie
 * arrive par lots, comme celle d'un script. L'exécution passe uniquement par
 * le store `telemetry` (sessions de flux).
 */
export function runCompose(
  project: string,
  action: ComposeAction,
  onEvent: (event: LogEvent) => void,
): Promise<ComposeRun> {
  const channel = new Channel<LogEvent>();
  channel.onmessage = onEvent;
  return call("compose_run", { project, action, onEvent: channel });
}

/** Interrompt une commande ; rend la main une fois le processus distant arrêté. */
export function stopCompose(runId: string): Promise<void> {
  return call("compose_stop", { runId });
}

/** Ce que « Arrêter et supprimer » supprime et conserve, pour le GO / NO GO. */
export function downInfo(project: string): Promise<DownInfo> {
  return call("compose_down_info", { project });
}

/** Dossier et fichiers de chaque projet, vérifiés par SFTP. */
export function checkProjects(): Promise<ProjectCheck[]> {
  return call("compose_check");
}

/** Oublie un projet descendu du profil connecté ; rien n'est touché sur le serveur. */
export function forgetProject(project: string): Promise<void> {
  return call("compose_forget", { project });
}
