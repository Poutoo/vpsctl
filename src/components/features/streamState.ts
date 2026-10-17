import type { TelemetryStream } from "../../stores/telemetry";

/** Forme d'état d'un flux (docs/06-design.md), jamais la couleur seule. */
export interface StreamState {
  shape: "disc" | "circle" | "triangle" | "half";
  tone: "success" | "muted" | "danger" | "warning";
  /** Libellé accessible de l'état. */
  label: string;
}

/**
 * État d'un flux pour son onglet :
 * - logs : en direct (cercle plein), terminé ou arrêté (cercle vide),
 *   interrompu par une erreur (triangle) ;
 * - script : en cours (demi-disque, comme une transition), réussi (cercle
 *   plein), en échec ou interrompu par une erreur (triangle), arrêté
 *   (cercle vide).
 */
export function streamState(stream: TelemetryStream): StreamState {
  const { status, exitCode } = stream.session;
  const script = stream.kind !== "logs";
  switch (status) {
    case "idle":
    case "starting":
    case "live":
      return script
        ? { shape: "half", tone: "warning", label: "en cours" }
        : { shape: "disc", tone: "success", label: "en direct" };
    case "ended":
      if (!script) return { shape: "circle", tone: "muted", label: "terminé" };
      return exitCode === 0
        ? { shape: "disc", tone: "success", label: "terminé · exit 0" }
        : {
            shape: "triangle",
            tone: "danger",
            label: `en échec · exit ${exitCode ?? "?"}`,
          };
    case "stopped":
      return { shape: "circle", tone: "muted", label: "interrompu" };
    case "error":
      return { shape: "triangle", tone: "danger", label: "flux interrompu" };
  }
}
