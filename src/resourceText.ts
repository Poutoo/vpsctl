/**
 * Textes de l'écran Ressources : formats des valeurs et bandeaux d'alerte
 * (par ordre de priorité : disque, mémoire, charge). Les niveaux viennent du
 * backend (hystérésis) ; ce module ne fait que les présenter.
 */
import type { ContainerUsage } from "./bindings/ContainerUsage";
import type { Level } from "./bindings/Level";
import type { Measure } from "./bindings/Measure";
import type { ResourceUpdate } from "./bindings/ResourceUpdate";
import { formatBytes } from "./format";

const LEVEL_RANK: Record<Level, number> = {
  nominal: 0,
  attention: 1,
  critical: 2,
};

export function worst(levels: Iterable<Level>): Level {
  let result: Level = "nominal";
  for (const level of levels) {
    if (LEVEL_RANK[level] > LEVEL_RANK[result]) result = level;
  }
  return result;
}

export function levelLabel(level: Level): string {
  return { nominal: "Nominal", attention: "Attention", critical: "Critique" }[
    level
  ];
}

export function valueOf<T>(measure: Measure<T>): T | null {
  return measure.status === "ok" ? measure.value : null;
}

/** Pourcentage arrondi comme pour les alertes : « 38 % ». */
export function formatPercent(percent: number): string {
  return `${Math.round(percent).toLocaleString("fr-FR")} %`;
}

/** Charge par cœur, au centième comme pour les alertes : « 0,68 ». */
export function formatLoad(value: number): string {
  return value.toLocaleString("fr-FR", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

/** Seuil de charge, au dixième : « 1,0 ». */
export function formatLoadThreshold(value: number): string {
  return value.toLocaleString("fr-FR", {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
}

/** Pourcentage de CPU d'un conteneur : « 22 % », « 2,0 % », « 0,6 % ». */
export function formatCpu(percent: number): string {
  const decimals = percent < 10 ? 1 : 0;
  return `${percent.toLocaleString("fr-FR", {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  })} %`;
}

/** Débit : « 1,8 Mo/s ». */
export function formatRate(bytesPerSecond: number): string {
  return `${formatBytes(Math.max(0, bytesPerSecond))}/s`;
}

const CLOCK = new Intl.DateTimeFormat("fr-FR", {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

/** Heure d'un relevé : « 14:32:49 ». */
export function formatTime(millis: number): string {
  return CLOCK.format(new Date(millis));
}

export function interfacesLabel(interfaces: readonly string[]): string {
  return `${interfaces.join(", ")} · reçu et émis`;
}

/**
 * Repère de navigation : pire niveau du disque et de la mémoire, plus la
 * charge quand l'écran est affiché (hors de l'écran, elle n'est pas relevée).
 */
export function navLevel(
  update: ResourceUpdate | null,
  onScreen: boolean,
): Level {
  if (!update) return "nominal";
  const levels = [
    ...update.levels.disks.map((d) => d.level),
    update.levels.memory,
  ];
  if (onScreen) levels.push(update.levels.load);
  return worst(levels);
}

export interface ResourceAlert {
  resource: "disk" | "memory" | "load";
  level: Exclude<Level, "nominal">;
  title: string;
  detail: string;
  /** Plus gros consommateur de mémoire (« Voir <conteneur> »). */
  container: ContainerUsage | null;
}

function diskAlert(update: ResourceUpdate): ResourceAlert | null {
  const disks = valueOf(update.disks) ?? [];
  const alerted = update.levels.disks
    .filter((d) => d.level !== "nominal")
    .map((d) => ({
      level: d.level as Exclude<Level, "nominal">,
      disk: disks.find((disk) => disk.mountPoint === d.mountPoint) ?? null,
      mountPoint: d.mountPoint,
    }))
    .sort((a, b) => {
      const byLevel = LEVEL_RANK[b.level] - LEVEL_RANK[a.level];
      if (byLevel !== 0) return byLevel;
      const pa = a.disk ? (valueOf(a.disk.space)?.percent ?? 0) : 0;
      const pb = b.disk ? (valueOf(b.disk.space)?.percent ?? 0) : 0;
      return pb - pa;
    });
  const first = alerted[0];
  if (!first) return null;
  const space = first.disk ? valueOf(first.disk.space) : null;
  const percent = space ? ` · ${formatPercent(space.percent)}` : "";
  const title =
    first.level === "critical"
      ? `Disque presque plein · ${first.mountPoint}${percent}`
      : `Disque bientôt plein · ${first.mountPoint}${percent}`;
  const parts: string[] = [];
  if (space) {
    parts.push(
      `${formatBytes(space.free)} libres sur ${formatBytes(space.total)}.`,
    );
  }
  if (first.mountPoint === update.dockerMount) {
    parts.push(
      update.automaticDockerUsage
        ? "Répartition de l'espace Docker ci-dessous."
        : "Analyse de l'espace Docker possible ci-dessous.",
    );
  }
  const others = alerted.slice(1).map((a) => a.mountPoint);
  if (others.length > 0) {
    parts.push(
      `${others.length > 1 ? "Autres points de montage" : "Autre point de montage"} en alerte : ${others.join(", ")}.`,
    );
  }
  return {
    resource: "disk",
    level: first.level,
    title,
    detail: parts.join(" "),
    container: null,
  };
}

function memoryAlert(update: ResourceUpdate): ResourceAlert | null {
  const level = update.levels.memory;
  if (level === "nominal") return null;
  const memory = valueOf(update.memory);
  const percent = memory ? ` · ${formatPercent(memory.percent)}` : "";
  const title = `${level === "critical" ? "Mémoire saturée" : "Mémoire élevée"}${percent}`;
  const containers = valueOf(update.containers) ?? [];
  const top = containers[0] ?? null;
  let detail = "";
  if (memory) {
    detail = `${formatBytes(memory.used)} utilisés sur ${formatBytes(memory.total)}`;
    const swap = memory.swap;
    if (swap && swap.total > 0 && swap.used > 0) {
      detail += ` ; swap entamé (${formatBytes(swap.used)} sur ${formatBytes(swap.total)})`;
    }
    detail += ".";
  }
  if (top) {
    detail += ` Plus gros consommateur : ${top.name}, ${formatBytes(top.memoryUsed)}.`;
  }
  return {
    resource: "memory",
    level,
    title,
    detail: detail.trim(),
    container: top,
  };
}

function loadAlert(update: ResourceUpdate): ResourceAlert | null {
  const level = update.levels.load;
  if (level === "nominal") return null;
  const load = valueOf(update.load);
  const value = load ? ` · ${formatLoad(load.perCore[1])} par cœur` : "";
  const cores = update.cores;
  return {
    resource: "load",
    level,
    title: `${level === "critical" ? "Charge très élevée" : "Charge élevée"}${value}`,
    detail:
      load && cores
        ? `Moyenne sur 5 min : ${formatLoadThreshold(load.raw[1])} pour ${cores} ${cores > 1 ? "cœurs" : "cœur"} ; sur 1 min : ${formatLoad(load.perCore[0])} par cœur.`
        : "",
    container: null,
  };
}

/** Bandeaux d'alerte, par ordre de priorité : disque, mémoire, charge. */
export function resourceAlerts(update: ResourceUpdate | null): ResourceAlert[] {
  if (!update) return [];
  return [diskAlert(update), memoryAlert(update), loadAlert(update)].filter(
    (a): a is ResourceAlert => a !== null,
  );
}
