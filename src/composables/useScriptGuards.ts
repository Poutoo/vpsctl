import { interruptConsequences } from "../composeActions";
import {
  streamName,
  useTelemetryStore,
  type TelemetryStream,
} from "../stores/telemetry";
import { useConfirm } from "./useConfirm";
import { useEditorActions } from "./useEditorActions";

/** Ce que l'interruption d'un script implique, rappelé dans chaque GO / NO GO. */
function interruption(): string[] {
  return [
    "SIGTERM est envoyé à tous ses processus sur le serveur.",
    "Ce qu'il a déjà fait n'est pas annulé.",
    "Sa dernière exécution est enregistrée comme interrompue.",
  ];
}

/** Script ou commande compose en cours : l'arrêter demande un GO / NO GO. */
function isRunningProcedure(tab: TelemetryStream): boolean {
  return tab.kind !== "logs" && tab.session.active;
}

/**
 * GO / NO GO avant d'interrompre un script ou une commande compose en
 * cours : bouton Interrompre, fermeture de son onglet, coupure de la liaison,
 * fermeture de l'application. Sans procédure en cours, tout est accepté sans
 * fenêtre.
 */
export function useScriptGuards() {
  const telemetry = useTelemetryStore();
  const { confirm } = useConfirm();
  const editor = useEditorActions();

  /** Bouton Interrompre de la console d'un script ou d'une commande compose. */
  async function confirmStop(tab: TelemetryStream): Promise<boolean> {
    if (!isRunningProcedure(tab)) return true;
    if (tab.kind === "compose") {
      return confirm({
        title: `Interrompre docker compose ${tab.action} ?`,
        subject: tab.project,
        consequences: interruptConsequences(tab.action),
        confirmLabel: "Interrompre",
        danger: true,
      });
    }
    return confirm({
      title: "Interrompre la procédure ?",
      subject: streamName(tab),
      consequences: interruption(),
      confirmLabel: "Interrompre",
      danger: true,
    });
  }

  /** Fermeture de l'onglet d'un script ou d'une commande en cours. */
  async function confirmCloseTab(tab: TelemetryStream): Promise<boolean> {
    if (!isRunningProcedure(tab)) return true;
    if (tab.kind === "compose") {
      return confirm({
        title: "Fermer l'onglet ?",
        subject: tab.project,
        consequences: [
          `docker compose ${tab.action} sera interrompu.`,
          ...interruptConsequences(tab.action),
        ],
        confirmLabel: "Fermer et interrompre",
        danger: true,
      });
    }
    return confirm({
      title: "Fermer l'onglet ?",
      subject: streamName(tab),
      consequences: ["Le script sera interrompu.", ...interruption()],
      confirmLabel: "Fermer et interrompre",
      danger: true,
    });
  }

  /**
   * Coupure de la liaison ou fermeture de l'application : d'abord le fichier
   * modifié dans l'éditeur, puis les procédures en cours.
   */
  async function confirmLeave(
    reason: "disconnect" | "close",
  ): Promise<boolean> {
    const exit = reason === "close" ? "window" : "disconnect";
    if (!(await editor.confirmExit(exit))) return false;
    const scripts = telemetry.runningScripts;
    const commands = telemetry.runningCompose;
    if (scripts.length === 0 && commands.length === 0) return true;
    const counts: string[] = [];
    if (scripts.length > 0) {
      counts.push(
        scripts.length > 1
          ? `${scripts.length} scripts en cours seront interrompus.`
          : "1 script en cours sera interrompu.",
      );
    }
    if (commands.length > 0) {
      counts.push(
        commands.length > 1
          ? `${commands.length} commandes compose en cours seront interrompues : un projet peut rester à moitié démarré ou arrêté.`
          : "1 commande compose en cours sera interrompue : son projet peut rester à moitié démarré ou arrêté.",
      );
    }
    const plural = scripts.length + commands.length > 1;
    const recorded =
      scripts.length > 1
        ? [
            "La dernière exécution de ces scripts est enregistrée comme interrompue.",
          ]
        : scripts.length === 1
          ? [
              "La dernière exécution du script est enregistrée comme interrompue.",
            ]
          : [];
    return confirm({
      title:
        reason === "disconnect"
          ? "Couper la liaison ?"
          : "Fermer l'application ?",
      subject: [...scripts, ...commands].map(streamName).join(", "),
      consequences: [
        ...counts,
        plural
          ? "SIGTERM est envoyé à tous leurs processus sur le serveur."
          : "SIGTERM est envoyé à tous ses processus sur le serveur.",
        "Ce qui est déjà fait n'est pas annulé.",
        ...recorded,
      ],
      confirmLabel: reason === "disconnect" ? "Couper la liaison" : "Fermer",
      danger: true,
    });
  }

  return { confirmStop, confirmCloseTab, confirmLeave };
}
