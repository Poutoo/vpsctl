/**
 * Courbes des ressources (SVG maison) : fonctions pures qui transforment des
 * séries datées en tracés. L'axe horizontal couvre les 30 dernières minutes,
 * « maintenant » à droite.
 */

export const HISTORY_MS = 30 * 60_000;

export interface Point {
  at: number;
  value: number;
}

/** Intervalle de temps (perte de liaison : zone « sans relevé »). */
export interface Span {
  from: number;
  /** `null` : en cours, jusqu'à maintenant. */
  to: number | null;
}

export interface Frame {
  from: number;
  to: number;
  width: number;
  height: number;
  /** Valeur au sommet du cadre ; au-delà, le tracé est écrêté. */
  max: number;
}

export function frameAt(
  now: number,
  width: number,
  height: number,
  max: number,
): Frame {
  return { from: now - HISTORY_MS, to: now, width, height, max };
}

const round = (n: number) => Math.round(n * 10) / 10;

export function xOf(at: number, frame: Frame): number {
  const ratio = (at - frame.from) / (frame.to - frame.from);
  return round(Math.min(1, Math.max(0, ratio)) * frame.width);
}

export function yOf(value: number, frame: Frame): number {
  const ratio = frame.max > 0 ? Math.min(1, Math.max(0, value / frame.max)) : 0;
  return round(frame.height - ratio * frame.height);
}

/**
 * Points en segments continus : un écart de plus de `maxGapMs` entre deux
 * points (mesure non relevée hors de l'écran, perte de liaison) interrompt
 * la ligne. Les points hors du cadre sont écartés.
 */
export function segments(
  points: readonly Point[],
  maxGapMs: number,
  from = -Infinity,
): Point[][] {
  const result: Point[][] = [];
  let current: Point[] = [];
  let previous: Point | null = null;
  for (const point of points) {
    if (point.at < from) continue;
    if (previous && point.at - previous.at > maxGapMs) {
      if (current.length > 0) result.push(current);
      current = [];
    }
    current.push(point);
    previous = point;
  }
  if (current.length > 0) result.push(current);
  return result;
}

/** Ligne : un sous-tracé par segment ; un point isolé devient un court trait. */
export function linePath(
  points: readonly Point[],
  frame: Frame,
  maxGapMs: number,
): string {
  return segments(points, maxGapMs, frame.from)
    .map((segment) => {
      const coords = segment.map(
        (p) => `${xOf(p.at, frame)},${yOf(p.value, frame)}`,
      );
      if (coords.length === 1) {
        const p = segment[0];
        return `M${coords[0]}H${round(Math.min(frame.width, xOf(p.at, frame) + 2))}`;
      }
      return `M${coords.join("L")}`;
    })
    .join("");
}

/** Surface sous la ligne, fermée sur la base de chaque segment. */
export function areaPath(
  points: readonly Point[],
  frame: Frame,
  maxGapMs: number,
): string {
  return segments(points, maxGapMs, frame.from)
    .filter((segment) => segment.length > 1)
    .map((segment) => {
      const coords = segment.map(
        (p) => `${xOf(p.at, frame)},${yOf(p.value, frame)}`,
      );
      const first = xOf(segment[0].at, frame);
      const last = xOf(segment[segment.length - 1].at, frame);
      return `M${first},${frame.height}L${coords.join("L")}L${last},${frame.height}Z`;
    })
    .join("");
}

/** Zones « sans relevé » visibles dans le cadre. */
export function spanRects(
  spans: readonly Span[],
  frame: Frame,
): { x: number; width: number }[] {
  return spans
    .map((span) => ({ from: span.from, to: span.to ?? frame.to }))
    .filter((span) => span.to > frame.from && span.from < frame.to)
    .map((span) => {
      const x = xOf(span.from, frame);
      return { x, width: round(xOf(span.to, frame) - x) };
    })
    .filter((rect) => rect.width > 0);
}

/**
 * Sommet d'échelle « rond » (1, 2, 2,5, 5 × 10ⁿ) au-dessus de la plus grande
 * valeur, jamais sous `minimum`.
 */
export function niceMax(values: readonly number[], minimum: number): number {
  const top = Math.max(minimum, ...values.filter(Number.isFinite));
  const power = 10 ** Math.floor(Math.log10(top));
  for (const step of [1, 2, 2.5, 5, 10]) {
    if (top <= step * power) return step * power;
  }
  return 10 * power;
}

/** Plus grande valeur des points du cadre (pic sur 30 min). */
export function peak(points: readonly Point[], from: number): number | null {
  let best: number | null = null;
  for (const point of points) {
    if (point.at >= from && (best === null || point.value > best))
      best = point.value;
  }
  return best;
}
