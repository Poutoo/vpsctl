import type { BackgroundChoice } from "../bindings/BackgroundChoice";
import type { Preferences } from "../bindings/Preferences";
import { call } from "./invoke";

// Préférences de l'interface (docs/04-contrat-commandes.md, « Préférences »).

export function getPreferences(): Promise<Preferences> {
  return call("preferences_get");
}

/** Assombrissement de l'image de fond, de 0 à 100. */
export function setDimming(dimming: number): Promise<Preferences> {
  return call("preferences_set_dimming", { dimming });
}

export function setOriginalColors(enabled: boolean): Promise<Preferences> {
  return call("preferences_set_original_colors", { enabled });
}

// Image de fond (docs/04-contrat-commandes.md, « Image de fond »). Les
// octets arrivent bruts (`ArrayBuffer`), sans passer par du JSON.

/** Fenêtre de sélection du système, puis contrôles de l'image choisie. */
export function chooseBackground(): Promise<BackgroundChoice> {
  return call("background_choose");
}

export function pendingBackgroundImage(token: string): Promise<ArrayBuffer> {
  return call("background_pending_image", { token });
}

/** Enregistre l'image en attente, une fois décodée par la webview. */
export function commitBackground(token: string): Promise<Preferences> {
  return call("background_commit", { token });
}

export function discardBackground(token: string): Promise<void> {
  return call("background_discard", { token });
}

export function backgroundImage(): Promise<ArrayBuffer> {
  return call("background_image");
}

/** Revient au fond d'origine et supprime la copie. */
export function resetBackground(): Promise<Preferences> {
  return call("background_reset");
}

/** Vrai une seule fois si la copie a été écartée au démarrage. */
export function backgroundLostAtStartup(): Promise<boolean> {
  return call("background_lost_at_startup");
}
