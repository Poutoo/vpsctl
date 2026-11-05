import type { ThresholdPair } from "./bindings/ThresholdPair";
import type { Thresholds } from "./bindings/Thresholds";

/**
 * Seuils d'alerte : mêmes règles que le backend (`resources::settings::validate`,
 * cas partagés `tests/shared/resource-thresholds.json`). Disque et mémoire :
 * entiers de 1 à 100 ; charge par cœur : de 0,1 à 64 au dixième ; attention
 * strictement inférieure à critique.
 */

export type Resource = keyof Thresholds;
export type ThresholdField = `${Resource}.${keyof ThresholdPair}`;

export const DEFAULT_THRESHOLDS: Thresholds = {
  disk: { attention: 80, critical: 90 },
  memory: { attention: 85, critical: 95 },
  load: { attention: 1, critical: 2 },
};

const LOAD_MAX = 64;

function percentProblem(value: number): string | null {
  return Number.isInteger(value) && value >= 1 && value <= 100
    ? null
    : "Un nombre entier de 1 à 100.";
}

function loadProblem(value: number): string | null {
  const tenths = value * 10;
  const valid =
    Number.isFinite(value) &&
    Math.abs(tenths - Math.round(tenths)) <= 1e-6 &&
    value >= 0.1 - 1e-9 &&
    value <= LOAD_MAX;
  return valid ? null : "De 0,1 à 64 par cœur, au dixième.";
}

/** Problème de chaque champ, `null` s'il est valide. */
export function thresholdProblems(
  thresholds: Thresholds,
): Record<ThresholdField, string | null> {
  const problems = {} as Record<ThresholdField, string | null>;
  for (const resource of ["disk", "memory", "load"] as const) {
    const pair = thresholds[resource];
    const check = resource === "load" ? loadProblem : percentProblem;
    problems[`${resource}.attention`] = check(pair.attention);
    problems[`${resource}.critical`] = check(pair.critical);
    if (
      problems[`${resource}.attention`] === null &&
      problems[`${resource}.critical`] === null &&
      pair.attention >= pair.critical
    ) {
      problems[`${resource}.attention`] = "Inférieur au seuil critique.";
    }
  }
  return problems;
}

export function thresholdsValid(thresholds: Thresholds): boolean {
  return Object.values(thresholdProblems(thresholds)).every((p) => p === null);
}

/** Saisie d'un seuil : virgule ou point décimal ; `NaN` si illisible. */
export function parseThreshold(text: string): number {
  const normalized = text.trim().replace(",", ".");
  return /^\d+(\.\d+)?$/.test(normalized) ? Number(normalized) : NaN;
}

export function sameThresholds(a: Thresholds, b: Thresholds): boolean {
  return (["disk", "memory", "load"] as const).every(
    (r) => a[r].attention === b[r].attention && a[r].critical === b[r].critical,
  );
}
