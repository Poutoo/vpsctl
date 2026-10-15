/**
 * Sections de l'application : libellé principal (terme technique réel),
 * numéral romain et surtitre (vocabulaire spatial, en couche secondaire).
 * Source unique pour la navigation et les en-têtes de page.
 */
export interface AppSection {
  route: "dashboard" | "files" | "scripts" | "settings";
  numeral: string;
  label: string;
  kicker: string;
}

export const SECTIONS: readonly AppSection[] = [
  {
    route: "dashboard",
    numeral: "I",
    label: "Tableau de bord",
    kicker: "État de la station",
  },
  { route: "files", numeral: "II", label: "Fichiers", kicker: "Itinéraire" },
  { route: "scripts", numeral: "III", label: "Scripts", kicker: "Procédures" },
  {
    route: "settings",
    numeral: "IV",
    label: "Réglages",
    kicker: "Profils de liaison",
  },
];

/** Section d'une route, `null` hors navigation (écran de connexion). */
export function sectionOf(routeName: unknown): AppSection | null {
  return SECTIONS.find((section) => section.route === routeName) ?? null;
}
