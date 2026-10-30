import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PresenceStatus } from "../bindings/PresenceStatus";
import { call } from "./invoke";

// Présence Discord (docs/04-contrat-commandes.md, « Présence Discord »).

export function presenceStatus(): Promise<PresenceStatus> {
  return call("presence_status");
}

/** Enregistre l'interrupteur et l'applique ; le statut suit par événement. */
export function setPresenceEnabled(enabled: boolean): Promise<void> {
  return call("presence_set_enabled", { enabled });
}

/** Nouvelle tentative immédiate de connexion à Discord. */
export function retryPresence(): Promise<void> {
  return call("presence_retry");
}

export function onPresenceStatus(
  handler: (status: PresenceStatus) => void,
): Promise<UnlistenFn> {
  return listen<PresenceStatus>("presence-status", (event) =>
    handler(event.payload),
  );
}
