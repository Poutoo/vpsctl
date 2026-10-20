import type { Service } from "../../bindings/Service";

/** Affichage de l'activation au démarrage d'un service. */
export interface StartupView {
  /** Position du curseur : activé au démarrage. */
  checked: boolean;
  /** Cause d'un état non modifiable, affichée avec un cadenas ; `null` : interrupteur actif. */
  locked: string | null;
  /** Explication en infobulle d'un état non modifiable. */
  help: string | null;
}

/** États bruts de `list-unit-files` qui ne se changent pas depuis l'appli. */
const OTHER_STATES: Record<string, { label: string; help: string }> = {
  static: {
    label: "Statique",
    help: "Pas de section [Install] : le service ne démarre que comme dépendance d'une autre unité.",
  },
  indirect: {
    label: "Indirect",
    help: "Activé par une autre unité (Also=) ou par une instance : à changer depuis celle-ci.",
  },
  generated: {
    label: "Généré",
    help: "Unité produite par un générateur de systemd : non modifiable.",
  },
  transient: {
    label: "Transitoire",
    help: "Unité temporaire, créée pendant l'exécution.",
  },
  alias: {
    label: "Alias",
    help: "Autre nom d'une unité : son activation suit celle de l'unité d'origine.",
  },
  "enabled-runtime": {
    label: "Activé (session)",
    help: "Activé jusqu'au prochain redémarrage seulement (enable --runtime).",
  },
  linked: {
    label: "Lié",
    help: "Fichier lié depuis un autre dossier (systemctl link).",
  },
  "linked-runtime": {
    label: "Lié (session)",
    help: "Fichier lié jusqu'au prochain redémarrage seulement.",
  },
  masked: {
    label: "Masqué",
    help: "Unité masquée : elle ne peut être ni démarrée ni activée.",
  },
  "masked-runtime": {
    label: "Masqué (session)",
    help: "Unité masquée jusqu'au prochain redémarrage.",
  },
  bad: {
    label: "Invalide",
    help: "Fichier d'unité que systemd ne sait pas lire.",
  },
};

/** États bruts où le service démarre avec le serveur. */
const ENABLED_STATES = new Set(["enabled", "enabled-runtime"]);

/**
 * Libellé et explication de l'interrupteur « Au démarrage », d'après la règle
 * calculée par le backend (`startupBlock`) et l'état brut.
 */
export function startupView(service: Service): StartupView {
  const state = service.startup;
  const checked = state !== null && ENABLED_STATES.has(state);
  const lock = (locked: string, help: string) => ({ checked, locked, help });
  switch (service.startupBlock) {
    case null:
      return { checked, locked: null, help: null };
    case "invalidName":
      return lock(
        "Non pilotable",
        "Nom hors du format accepté : aucune commande possible sur ce service.",
      );
    case "masked":
      return lock("Masqué", OTHER_STATES.masked.help);
    case "templateInstance":
      return lock(
        "Instance",
        "Instance d'un gabarit : son activation au démarrage n'est pas gérée par Atlas.",
      );
    case "linked":
      return lock(
        "Lié",
        "Fichier lié depuis un autre dossier (systemctl link) : le désactiver supprimerait aussi le lien.",
      );
    case "unknown":
      return lock(
        "Inconnu",
        "État d'activation illisible (systemctl --user list-unit-files).",
      );
    case "otherState": {
      const known = state === null ? undefined : OTHER_STATES[state];
      return known
        ? lock(known.label, known.help)
        : lock(
            state ?? "Inconnu",
            `État « ${state ?? "inconnu"} » : non modifiable depuis Atlas.`,
          );
    }
  }
}
