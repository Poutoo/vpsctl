/**
 * Actions de projet compose (fonctions pures) : disponibilité, libellés,
 * contenu du GO / NO GO de « Arrêter et supprimer » et de l'interruption.
 */
import type { ComposeAction } from "./bindings/ComposeAction";
import type { ComposeProject } from "./bindings/ComposeProject";
import type { Container } from "./bindings/Container";
import type { DownInfo } from "./bindings/DownInfo";
import type { FilesStatus } from "./bindings/FilesStatus";
import type { AppIconName } from "./components/ui/AppIcon.vue";
import type { ConfirmOptions } from "./composables/useConfirm";
import { baseName, filesProblem, problemText } from "./composeView";

export interface ComposeActionMeta {
  action: ComposeAction;
  icon: AppIconName;
  label: string;
  /** Deuxième ligne de l'infobulle : ce que fait l'action. */
  hint: string;
}

/** Actions avant le séparateur, dans l'ordre de la maquette. */
export const PROJECT_ACTIONS: ComposeActionMeta[] = [
  {
    action: "up",
    icon: "play",
    label: "Démarrer",
    hint: "docker compose up -d : crée ou met à jour les conteneurs d'après le fichier compose et les démarre.",
  },
  {
    action: "pull",
    icon: "download",
    label: "Récupérer les images",
    hint: "docker compose pull : télécharge les nouvelles images, sans toucher aux conteneurs. Pour les utiliser : Démarrer.",
  },
  {
    action: "restart",
    icon: "refresh",
    label: "Redémarrer",
    hint: "Relance les conteneurs sans appliquer les changements du fichier compose ni les nouvelles images. Pour les appliquer : Démarrer.",
  },
];

/** Action isolée par le séparateur, comme toute suppression. */
export const DOWN_ACTION: ComposeActionMeta = {
  action: "down",
  icon: "trash",
  label: "Arrêter et supprimer",
  hint: "docker compose down : arrête et supprime les conteneurs et le réseau du projet. Volumes et images conservés.",
};

export const COMPOSE_SUCCESS_TITLE: Record<ComposeAction, string> = {
  up: "Projet démarré",
  pull: "Images récupérées",
  restart: "Projet redémarré",
  down: "Projet arrêté et supprimé",
};

export const COMPOSE_FAILURE_TITLE: Record<ComposeAction, string> = {
  up: "Échec de docker compose up",
  pull: "Échec de docker compose pull",
  restart: "Échec de docker compose restart",
  down: "Échec de docker compose down",
};

/**
 * Raison d'indisponibilité de chaque action, ou `null`. Démarrer et
 * Récupérer ont besoin d'étiquettes utilisables et de fichiers présents et
 * lisibles ; Redémarrer et Arrêter passent par le seul nom du projet et ont
 * besoin d'au moins un conteneur.
 */
export function projectActionReasons(
  project: ComposeProject,
  check: FilesStatus | null,
  context: { lost: boolean; busy: boolean },
): Record<ComposeAction, string | null> {
  const all = (reason: string) => ({
    up: reason,
    pull: reason,
    restart: reason,
    down: reason,
  });
  if (context.lost) return all("liaison perdue");
  if (context.busy) return all("commande compose en cours sur ce projet");
  if (project.problem?.kind === "invalidName") {
    return all(problemText(project.problem));
  }
  const files = project.problem
    ? problemText(project.problem)
    : (filesProblem(check, project.files?.workingDir ?? "")?.short ?? null);
  const empty = project.state === "down" ? "aucun conteneur" : null;
  return { up: files, pull: files, restart: empty, down: empty };
}

/** Libellé « en transition » d'un conteneur pendant une commande, ou `null`. */
export function containerProgress(
  action: ComposeAction,
  container: Container,
): string | null {
  switch (action) {
    case "up":
      return container.dockerState === "running" ? null : "Démarrage";
    case "restart":
      return "Redémarrage";
    case "down":
      return "Arrêt";
    case "pull":
      return null;
  }
}

/** Ce que l'interruption implique, selon la commande. */
export function interruptConsequences(action: ComposeAction): string[] {
  const done: Record<ComposeAction, string> = {
    up: "Le projet peut rester à moitié démarré : certains conteneurs créés ou démarrés, d'autres non. Démarrer termine le travail.",
    pull: "Les images déjà récupérées restent ; les autres gardent leur ancienne version.",
    restart: "Une partie des conteneurs peut avoir redémarré, les autres non.",
    down: "Une partie des conteneurs peut déjà être supprimée, les autres tournent encore.",
  };
  return [
    "SIGTERM est envoyé à docker compose sur le serveur.",
    "Ce qui est déjà fait n'est pas annulé.",
    done[action],
  ];
}

/** Liste courte : au plus `max` éléments, puis « +N ». */
function shortList(items: string[], max = 6): string {
  const shown = items.slice(0, max).join(" · ");
  return items.length > max ? `${shown} · +${items.length - max}` : shown;
}

/** Nom de volume anonyme (64 caractères hexadécimaux) raccourci à 12. */
function shortVolume(name: string): string {
  return /^[0-9a-f]{64}$/.test(name) ? name.slice(0, 12) : name;
}

/** Message exact de la fenêtre, à la place de « réversible par up ». */
export const DOWN_WARNING =
  "Les conteneurs seront recréés par Démarrer et les volumes nommés sont conservés, mais les données écrites hors des volumes nommés sont perdues.";

/** GO / NO GO de « Arrêter et supprimer », d'après `compose_down_info`. */
export function downConfirmation(info: DownInfo): ConfirmOptions {
  const running = info.containers.filter((c) => c.running).length;
  const count = info.containers.length;
  const removals = [
    {
      label: `${count} ${count > 1 ? "conteneurs arrêtés puis supprimés" : "conteneur arrêté puis supprimé"} · ${running} en marche`,
      detail: shortList(info.containers.map((c) => c.service ?? c.name)),
    },
  ];
  if (info.networks.length > 0) {
    removals.push({
      label:
        info.networks.length > 1
          ? "Réseaux du projet supprimés"
          : "Réseau du projet supprimé",
      detail: shortList(info.networks),
    });
  }
  if (info.anonymousVolumes.length > 0) {
    const n = info.anonymousVolumes.length;
    removals.push({
      label:
        n > 1
          ? `${n} volumes anonymes détachés : conservés sur le serveur, mais plus rattachés au projet après le down`
          : "1 volume anonyme détaché : conservé sur le serveur, mais plus rattaché au projet après le down",
      detail: shortList(info.anonymousVolumes.map(shortVolume)),
    });
  }

  const checks = [
    info.namedVolumes.length > 0
      ? {
          label: "Volumes nommés conservés · down sans --volumes",
          detail: shortList(info.namedVolumes),
        }
      : { label: "Aucun volume nommé monté" },
  ];
  if (info.binds.length > 0) {
    checks.push({
      label: "Dossiers du serveur montés conservés",
      detail: shortList(info.binds, 4),
    });
  }
  checks.push({ label: "Images conservées · down sans --rmi" });

  const workingDir = info.files?.workingDir ?? "";
  const problem = filesProblem(info.filesStatus, workingDir);
  const relaunch = !info.files
    ? "Relance impossible depuis Atlas : dossier et fichiers du projet inconnus."
    : problem
      ? `Relance impossible depuis Atlas : ${problem.text}`
      : `Relance possible avec Démarrer, ${info.files.configFiles.length > 1 ? "les fichiers compose sont présents" : "le fichier compose est présent"} : ${info.files.configFiles.map(baseName).join(" + ")}.`;

  return {
    title: "Arrêter et supprimer les conteneurs du projet ?",
    subject: info.files ? `${info.project} · ${workingDir}` : info.project,
    command: info.command,
    aside: "Données hors volumes perdues",
    removals,
    checks,
    consequences: [relaunch],
    warning: DOWN_WARNING,
    confirmLabel: "Arrêter et supprimer",
    danger: true,
  };
}
