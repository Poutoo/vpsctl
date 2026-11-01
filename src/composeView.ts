/**
 * Projets compose dans la section Docker (fonctions pures) : regroupement
 * des conteneurs pour l'affichage, filtre, libellés d'état et de détail.
 */
import type { ComposeProject } from "./bindings/ComposeProject";
import type { Container } from "./bindings/Container";
import type { FilesStatus } from "./bindings/FilesStatus";
import type { ProjectProblem } from "./bindings/ProjectProblem";
import type { ProjectState } from "./bindings/ProjectState";
import { matchesFilter } from "./listFilter";

/** Groupe affiché : un projet et ses conteneurs, ou les conteneurs hors compose. */
export type ContainerGroup =
  | { kind: "project"; project: ComposeProject; containers: Container[] }
  | { kind: "loose"; containers: Container[] };

export const PROJECT_STATE_LABEL: Record<ProjectState, string> = {
  nominal: "Nominal",
  partial: "Partiel",
  failed: "En échec",
  transition: "En transition",
  stopped: "Arrêté",
  down: "Descendu",
};

/** Forme de chaque état (`StatusIcon`) : jamais la couleur seule. */
export const PROJECT_STATE_SHAPE = {
  nominal: "disc",
  partial: "target",
  failed: "triangle",
  transition: "half",
  stopped: "circle",
  down: "circle",
} as const satisfies Record<ProjectState, string>;

/** « Nominal · 2/2 », « Descendu · 0/4 ». */
export function projectCount(project: ComposeProject): string {
  return `${project.running}/${project.total}`;
}

/** Dernier composant d'un chemin. */
export function baseName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1) || path;
}

/** Pourquoi les étiquettes d'un projet sont inutilisables. */
export function problemText(problem: ProjectProblem): string {
  switch (problem.kind) {
    case "invalidName":
      return "nom de projet hors du format de compose";
    case "incomplete":
      return "étiquettes du dossier ou des fichiers absentes";
    case "invalidPath":
      return `chemin refusé : ${problem.path}`;
    case "ambiguousPath":
      return `virgule dans un chemin de l'étiquette ${problem.label}`;
    case "divergent":
      return "étiquettes différentes d'un conteneur à l'autre";
  }
}

/**
 * Colonne Détail d'un projet : fichiers de configuration (noms seuls),
 * « aucun conteneur · mémorisé » pour un projet descendu, ou le problème
 * des étiquettes.
 */
export function projectDetail(project: ComposeProject): string {
  if (project.problem) return problemText(project.problem);
  if (project.state === "down") return "aucun conteneur · mémorisé";
  return (project.files?.configFiles ?? []).map(baseName).join(" + ");
}

/** Infobulle de la colonne Détail : chemins complets. */
export function projectDetailHelp(project: ComposeProject): string {
  const files = project.files;
  if (!files) return projectDetail(project);
  const lines = files.configFiles.map((file) => `-f ${file}`);
  for (const env of files.envFiles) lines.push(`--env-file ${env}`);
  return lines.join("\n");
}

/** Dossier ou fichier qui empêche Démarrer et Récupérer les images. */
export interface FilesProblem {
  /** Colonne Détail : « dossier introuvable ». */
  short: string;
  /** Ligne d'explication, phrase en gras. */
  text: string;
  /** Le dossier lui-même manque : son chemin est barré. */
  dirMissing: boolean;
}

/**
 * Problème révélé par la vérification des fichiers, ou `null` (vérifiés,
 * pas encore vérifiés, ou vérification impossible : l'action vérifie de
 * nouveau avant de partir).
 */
export function filesProblem(
  status: FilesStatus | null | undefined,
  workingDir: string,
): FilesProblem | null {
  if (!status) return null;
  const problem = (short: string, text: string, dirMissing = false) => ({
    short,
    text,
    dirMissing,
  });
  switch (status.kind) {
    case "ok":
    case "unchecked":
      return null;
    case "dirMissing":
      return problem(
        "dossier introuvable",
        `${workingDir} n'existe plus sur le serveur.`,
        true,
      );
    case "notDir":
      return problem(
        "dossier invalide",
        `${workingDir} n'est pas un dossier.`,
        true,
      );
    case "missing":
      return problem("fichier introuvable", `${status.path} est introuvable.`);
    case "notFile":
      return problem(
        "fichier invalide",
        `${status.path} n'est pas un fichier.`,
      );
    case "unreadable":
      return problem(
        "fichier illisible",
        `${status.path} est illisible pour le compte connecté.`,
      );
  }
}

/**
 * Groupes affichés, dans l'ordre : projets (triés par nom, descendus
 * compris), puis les conteneurs hors compose. Un projet dont le nom ou le
 * dossier correspond au filtre montre tous ses conteneurs ; sinon seulement
 * ceux qui correspondent, et il disparaît s'il n'y en a aucun. Renvoie aussi
 * le nombre de conteneurs affichés.
 */
export function groupContainers(
  containers: readonly Container[],
  projects: readonly ComposeProject[],
  query: string,
): { groups: ContainerGroup[]; shown: number } {
  const containerMatches = (c: Container) =>
    matchesFilter([c.name, c.image], query);
  const groups: ContainerGroup[] = [];
  let shown = 0;
  const known = new Set(projects.map((p) => p.name));

  for (const project of projects) {
    const members = containers.filter((c) => c.project === project.name);
    const whole = matchesFilter(
      [project.name, project.files?.workingDir ?? null],
      query,
    );
    const visible = whole ? members : members.filter(containerMatches);
    if (!whole && visible.length === 0) continue;
    groups.push({ kind: "project", project, containers: visible });
    shown += visible.length;
  }

  const loose = containers
    .filter((c) => c.project === null || !known.has(c.project))
    .filter(containerMatches);
  if (loose.length > 0) {
    groups.push({ kind: "loose", containers: loose });
    shown += loose.length;
  }
  return { groups, shown };
}
