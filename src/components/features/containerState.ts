import type { Container } from "../../bindings/Container";
import { formatBytes } from "../../format";

/** Code de sortie d'un SIGKILL : arrêt forcé ou manque de mémoire. */
const SIGKILL_EXIT = 137;

/**
 * Infobulle de l'état d'un conteneur. Les deux cas du code 137 sont toujours
 * expliqués : la cause est lue par `docker inspect`, jamais devinée.
 */
export function containerStateHelp(container: Container): string {
  const raw = `${container.dockerState} · ${container.status}`;
  if (container.oomKilled) {
    const limit =
      container.memoryLimit === null
        ? "faute de mémoire disponible sur le serveur"
        : `à la limite de ${formatBytes(container.memoryLimit)}`;
    return `Tué par manque de mémoire : le noyau a arrêté le processus ${limit} (OOMKilled). Code de sortie ${SIGKILL_EXIT}. ${raw}`;
  }
  if (
    container.dockerState === "exited" &&
    container.exitCode === SIGKILL_EXIT
  ) {
    return `Arrêté par SIGKILL, sans manque de mémoire : docker stop a dépassé son délai, ou kill -9. ${raw}`;
  }
  return raw;
}

/** Complément affiché après l'état : code de sortie d'un conteneur arrêté ou en échec. */
export function containerStateDetail(container: Container): string | null {
  if (container.oomKilled) return null;
  const shown = container.state === "stopped" || container.state === "failed";
  return shown && container.exitCode !== null
    ? `code ${container.exitCode}`
    : null;
}
