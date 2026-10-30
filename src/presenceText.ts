import type { LinkPhase } from "./bindings/LinkPhase";
import type { PresenceState } from "./bindings/PresenceState";

// Textes du panneau Discord Rich Presence (planche VI), par état.

export interface PresenceNotice {
  tone: "info" | "success" | "warning";
  title: string;
  text: string;
  /** Bouton « Réessayer ». */
  retry: boolean;
}

/** Libellés envoyés à Discord (`presence/activity.rs`). */
export const PHASE_LABEL: Record<LinkPhase, string> = {
  linked: "En liaison",
  ground: "Au sol",
};

export const APP_NAME = "Atlas · VPS Control";
/** Bouton de la carte vers le dépôt GitHub (`presence/activity.rs`). */
export const SOURCE_LABEL = "Source Code";

/** Statut à droite du bandeau. */
export function presenceBadge(state: PresenceState): string {
  switch (state) {
    case "off":
      return "Désactivé";
    case "searching":
      return "Actif · Discord absent";
    case "connected":
      return "Actif · Connecté";
    case "devUnavailable":
    case "notConfigured":
      return "Indisponible";
  }
}

export function presenceNotice(state: PresenceState): PresenceNotice {
  switch (state) {
    case "off":
      return {
        tone: "info",
        title: "Désactivé",
        text: "Rien n'est partagé avec Discord.",
        retry: false,
      };
    case "searching":
      return {
        tone: "warning",
        title: "Discord non détecté",
        text: "Lancez Discord sur cet ordinateur : la carte sera publiée dès qu'il répond.",
        retry: true,
      };
    case "connected":
      return {
        tone: "success",
        title: "Connecté à Discord",
        text: "La carte ci-dessous est affichée sur votre profil.",
        retry: false,
      };
    case "devUnavailable":
      return {
        tone: "info",
        title: "Indisponible en mode développement",
        text: "La présence Discord n'existe que dans l'application installée.",
        retry: false,
      };
    case "notConfigured":
      return {
        tone: "info",
        title: "Indisponible dans cette version",
        text: "L'identifiant de l'application Discord n'est pas renseigné : rien ne peut être publié.",
        retry: false,
      };
  }
}

/** Étiquette de l'aperçu de la carte. */
export function previewKicker(state: PresenceState): string {
  switch (state) {
    case "off":
      return "Aperçu · non publié";
    case "searching":
      return "Aperçu · en attente de Discord";
    case "connected":
      return "Aperçu · publié sur Discord";
    case "devUnavailable":
    case "notConfigured":
      return "Aperçu · indisponible";
  }
}
