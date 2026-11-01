import type { ServerInfo } from "./bindings/ServerInfo";

/** Résumé de la barre d'état : « Ubuntu 26.04 LTS · Docker 29.6.1 · Compose 5.3.1 · systemd 259 ». */
export function serverSummary(info: ServerInfo): string {
  const compose =
    info.compose.status === "present"
      ? `Compose ${info.compose.version}`
      : info.compose.status === "absent"
        ? "Compose absent"
        : null;
  return [
    info.os,
    info.docker && `Docker ${info.docker}`,
    compose,
    info.systemd && `systemd ${info.systemd}`,
  ]
    .filter(Boolean)
    .join(" · ");
}
