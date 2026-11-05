import { Channel } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AnalysisEvent } from "../bindings/AnalysisEvent";
import type { AnalysisKind } from "../bindings/AnalysisKind";
import type { ResourceUpdate } from "../bindings/ResourceUpdate";
import type { Thresholds } from "../bindings/Thresholds";
import { call } from "./invoke";

// Ressources du serveur (docs/04-contrat-commandes.md, « Ressources du serveur »).

/** Écran Ressources affiché et fenêtre visible : relevé complet toutes les 5 s. */
export function watchResources(foreground: boolean): Promise<void> {
  return call("resources_watch", { foreground });
}

/** Dernier relevé de la session (rechargement de la page). */
export function resourcesState(): Promise<ResourceUpdate | null> {
  return call("resources_state");
}

export function thresholds(profileId: string): Promise<Thresholds> {
  return call("resource_thresholds_get", { profileId });
}

export function saveThresholds(
  profileId: string,
  thresholds: Thresholds,
): Promise<Thresholds> {
  return call("resource_thresholds_save", { profileId, thresholds });
}

/** « Valeurs par défaut » : ce profil seulement. */
export function resetThresholds(profileId: string): Promise<Thresholds> {
  return call("resource_thresholds_reset", { profileId });
}

/**
 * Analyse Docker à la demande (registre des flux du backend). Le résultat
 * arrive par un seul événement final ; une perte de liaison n'en envoie aucun.
 */
export function analyzeDocker(
  kind: AnalysisKind,
  onEvent: (event: AnalysisEvent) => void,
): Promise<string> {
  const channel = new Channel<AnalysisEvent>();
  channel.onmessage = onEvent;
  return call("docker_analyze", { kind, onEvent: channel });
}

/** Annule une analyse : TERM au groupe de processus, 3 s au plus. */
export function stopAnalysis(runId: string): Promise<void> {
  return call("docker_analyze_stop", { runId });
}

export function onResourcesUpdate(
  handler: (update: ResourceUpdate) => void,
): Promise<UnlistenFn> {
  return listen<ResourceUpdate>("resources-update", (event) =>
    handler(event.payload),
  );
}
