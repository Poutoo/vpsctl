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
