import { useTelemetryStore, type TelemetryStream } from "../stores/telemetry";
import { useConfirm } from "./useConfirm";

/** Ce que l'interruption implique, rappelé dans chaque GO / NO GO. */
function interruption(plural = false): string[] {
  return plural
    ? [
        "SIGTERM est envoyé à tous leurs processus sur le serveur.",
        "Ce qu'ils ont déjà fait n'est pas annulé.",
        "Leur dernière exécution est enregistrée comme interrompue.",
      ]
    : [
        "SIGTERM est envoyé à tous ses processus sur le serveur.",
        "Ce qu'il a déjà fait n'est pas annulé.",
        "Sa dernière exécution est enregistrée comme interrompue.",
      ];
}

/**
 * GO / NO GO avant d'interrompre un script en cours : bouton Interrompre,
 * fermeture de son onglet, coupure de la liaison, fermeture de
 * l'application. Sans script en cours, tout est accepté sans fenêtre.
 */
export function useScriptGuards() {
  const telemetry = useTelemetryStore();
  const { confirm } = useConfirm();

  function isRunningScript(tab: TelemetryStream): boolean {
    return tab.kind === "script" && tab.session.active;
  }

  /** Bouton Interrompre de la console d'un script. */
  async function confirmStop(tab: TelemetryStream): Promise<boolean> {
    if (!isRunningScript(tab)) return true;
    return confirm({
      title: "Interrompre la procédure ?",
      subject: tab.kind === "script" ? tab.script.name : undefined,
      consequences: interruption(),
      confirmLabel: "Interrompre",
      danger: true,
    });
  }

  /** Fermeture de l'onglet d'un script en cours. */
  async function confirmCloseTab(tab: TelemetryStream): Promise<boolean> {
    if (!isRunningScript(tab)) return true;
    return confirm({
      title: "Fermer l'onglet ?",
      subject: tab.kind === "script" ? tab.script.name : undefined,
      consequences: ["Le script sera interrompu.", ...interruption()],
      confirmLabel: "Fermer et interrompre",
      danger: true,
    });
  }

  /** Coupure de la liaison ou fermeture de l'application. */
  async function confirmLeave(
    reason: "disconnect" | "close",
  ): Promise<boolean> {
    const running = telemetry.runningScripts;
    if (running.length === 0) return true;
    const count =
      running.length > 1
        ? `${running.length} scripts en cours seront interrompus`
        : "1 script en cours sera interrompu";
    const names = running
      .map((tab) => (tab.kind === "script" ? tab.script.name : ""))
      .join(", ");
    return confirm({
      title:
        reason === "disconnect"
          ? "Couper la liaison ?"
          : "Fermer l'application ?",
      subject: names,
      consequences: [`${count}.`, ...interruption(running.length > 1)],
      confirmLabel: reason === "disconnect" ? "Couper la liaison" : "Fermer",
      danger: true,
    });
  }

  return { confirmStop, confirmCloseTab, confirmLeave };
}
