/** Textes du LOS (planche III) : cause et durées. */
import type { LossCause } from "./bindings/LossCause";
import { formatClock } from "./format";

const CAUSE_LABEL: Record<LossCause, string> = {
  closed: "connexion fermée par le serveur",
  reset: "connexion réinitialisée par l'hôte",
  timeout: "le serveur ne répond plus",
  network: "réseau injoignable",
  unknown: "liaison interrompue",
};

/** Cause affichée, suivie du message du serveur ou du système s'il y en a un. */
export function causeLabel(cause: LossCause, detail: string | null): string {
  const label = CAUSE_LABEL[cause];
  return detail ? `${label} (${detail})` : label;
}

/** Durée d'une coupure : « 00:17 », « 72:05 ». */
export function outageLabel(ms: number): string {
  return formatClock(ms / 1000);
}

/** Heure locale « 09:41:07 ». */
export function timeLabel(millis: number): string {
  return new Date(millis).toLocaleTimeString("fr-FR", { hour12: false });
}
