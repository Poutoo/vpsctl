import { Channel } from "@tauri-apps/api/core";
import type { LogEvent } from "../bindings/LogEvent";
import type { LogSource } from "../bindings/LogSource";
import { call } from "./invoke";

// Logs en direct (docs/04-contrat-commandes.md). À utiliser uniquement via
// le composable `useLogStream`, qui arrête le flux au démontage.

/**
 * Ouvre un flux de logs : `tail` lignes d'historique (5 000 au plus), puis
 * les nouvelles lignes, par lots. Renvoie l'identifiant du flux.
 */
export function startLogs(
  source: LogSource,
  tail: number,
  onEvent: (event: LogEvent) => void,
): Promise<string> {
  const channel = new Channel<LogEvent>();
  channel.onmessage = onEvent;
  return call("logs_start", { source, tail, onEvent: channel });
}

/** Arrête un flux ; rend la main une fois le processus distant arrêté. */
export function stopLogs(streamId: string): Promise<void> {
  return call("logs_stop", { streamId });
}
