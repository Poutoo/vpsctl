import { Channel } from "@tauri-apps/api/core";
import type { LogEvent } from "../bindings/LogEvent";
import type { Script } from "../bindings/Script";
import { call } from "./invoke";

// Scripts personnalisés (docs/04-contrat-commandes.md). L'exécution passe
// uniquement par le store `telemetry` (sessions de flux).

export function listScripts(): Promise<Script[]> {
  return call("scripts_list");
}

/** `id` vide pour créer ; `lastRun` est ignoré (écrit par le backend seul). */
export function saveScript(script: Script): Promise<Script> {
  return call("scripts_save", { script });
}

export function deleteScript(id: string): Promise<void> {
  return call("scripts_delete", { id });
}

/**
 * Exécute un script dans le shell de connexion de l'utilisateur ; sa sortie
 * arrive par lots, comme les logs. Renvoie l'identifiant de l'exécution.
 */
export function runScript(
  id: string,
  onEvent: (event: LogEvent) => void,
): Promise<string> {
  const channel = new Channel<LogEvent>();
  channel.onmessage = onEvent;
  return call("script_run", { id, onEvent: channel });
}

/** Interrompt une exécution ; rend la main une fois le processus distant arrêté. */
export function stopScript(runId: string): Promise<void> {
  return call("script_stop", { runId });
}
