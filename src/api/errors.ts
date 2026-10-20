import type { AppError } from "../bindings/AppError";
import type { FileSide } from "../bindings/FileSide";
import type { ServiceStep } from "../bindings/ServiceStep";
import { formatBytes } from "../format";

/** Normalise une erreur reçue d'`invoke()` en `AppError`. */
export function toAppError(error: unknown): AppError {
  if (
    typeof error === "object" &&
    error !== null &&
    "kind" in error &&
    typeof error.kind === "string"
  ) {
    return error as AppError;
  }
  return { kind: "Io", message: String(error) };
}

/** Message compréhensible pour chaque `kind` d'`AppError`. */
export function errorMessage(error: AppError): string {
  switch (error.kind) {
    case "NotConnected":
      return "Aucune liaison active avec le serveur.";
    case "LinkLost":
      return "Liaison perdue avec le serveur.";
    case "UnknownHostKey":
      return "Clé du serveur inconnue : comparez son empreinte avant de l'accepter.";
    case "HostKeyMismatch":
      return "L'empreinte du serveur a changé : la liaison a été bloquée.";
    case "PassphraseRequired":
      return "La clé privée est chiffrée : sa passphrase est requise.";
    case "PassphraseIncorrect":
      return "Passphrase incorrecte.";
    case "AuthFailed":
      return "Authentification refusée : le serveur n'accepte pas cette clé pour cet utilisateur.";
    case "Cancelled":
      return "Connexion interrompue.";
    case "Network":
    case "InvalidInput":
    case "Io":
      return error.message;
    case "Remote":
      return `La commande distante a échoué (code ${error.exitCode}) : ${error.stderr}`;
    case "DockerNotInstalled":
      return "Docker n'est pas installé sur le serveur.";
    case "DockerPermissionDenied":
      return "Accès à Docker refusé : le compte SSH doit appartenir au groupe docker.";
    case "PermissionDenied":
      return `Accès refusé ${where(error.side)} : ${error.path}`;
    case "NotFound":
      return `Élément introuvable ${where(error.side)} : ${error.path}`;
    case "AlreadyExists":
      return error.isDir
        ? `Un dossier porte déjà ce nom : ${error.path}`
        : `Un élément porte déjà ce nom : ${error.path}`;
    case "InsufficientSpace":
      return `Espace disque insuffisant ${where(error.side)}${spaceDetail(error.needed, error.available)}.`;
    case "ServiceStepFailed":
      // La cause, puis l'état dans lequel la séquence laisse le service.
      return `Arrêt à l'étape « ${STEP_LABEL[error.step]} » : ${errorMessage(error.cause).replace(/\.?$/, ".")} ${error.state}`;
  }
}

/** Étapes de création et de suppression d'un service, telles qu'affichées. */
const STEP_LABEL: Record<ServiceStep, string> = {
  reload: "rechargement de systemd",
  enable: "activation et démarrage",
  stop: "arrêt",
  disable: "désactivation",
  removeDropIn: "suppression du dossier de surcharge",
  removeFile: "suppression du fichier",
  resetFailed: "réinitialisation de l'état d'échec",
};

function where(side: FileSide): string {
  return side === "remote" ? "sur le VPS" : "sur cette machine";
}

function spaceDetail(needed: number | null, available: number | null): string {
  if (needed === null) return "";
  const free = available === null ? "" : `, ${formatBytes(available)} libres`;
  return ` (${formatBytes(needed)} nécessaires${free})`;
}
