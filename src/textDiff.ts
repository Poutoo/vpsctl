/**
 * Différence ligne à ligne (fonctions pures) : algorithme de Myers, puis
 * comparaison à trois versions (ouverture, serveur, vous) qui attribue
 * chaque changement au serveur ou à vous.
 *
 * Les textes sont découpés sur `\n` sans rien retirer : un fichier terminé
 * par un retour à la ligne a une dernière « ligne » vide, ce qui rend
 * visible l'ajout ou le retrait du retour final.
 */

/** Plage modifiée : lignes `[aStart, aEnd)` de A remplacées par `[bStart, bEnd)` de B. */
export interface Hunk {
  aStart: number;
  aEnd: number;
  bStart: number;
  bEnd: number;
}

/**
 * Au-delà de ce nombre de modifications, la partie différente forme un seul
 * bloc : la mémoire de la trace croît avec le carré de ce nombre.
 */
const MAX_EDITS = 2000;

export function splitLines(text: string): string[] {
  return text.split("\n");
}

/** Plages modifiées de A vers B, dans l'ordre. */
export function diffLines(a: readonly string[], b: readonly string[]): Hunk[] {
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) {
    start += 1;
  }
  let aEnd = a.length;
  let bEnd = b.length;
  while (aEnd > start && bEnd > start && a[aEnd - 1] === b[bEnd - 1]) {
    aEnd -= 1;
    bEnd -= 1;
  }
  if (start === aEnd && start === bEnd) return [];
  if (start === aEnd || start === bEnd) {
    return [{ aStart: start, aEnd, bStart: start, bEnd }];
  }
  return myers(a, b, start, aEnd, start, bEnd);
}

/**
 * Myers (« An O(ND) Difference Algorithm », 1986) sur `a[aLo..aHi)` et
 * `b[bLo..bHi)`. `trace[d]` garde l'état avant l'étape `d`, pour retrouver
 * le chemin à rebours.
 */
function myers(
  a: readonly string[],
  b: readonly string[],
  aLo: number,
  aHi: number,
  bLo: number,
  bHi: number,
): Hunk[] {
  const n = aHi - aLo;
  const m = bHi - bLo;
  const max = Math.min(n + m, MAX_EDITS);
  const offset = max + 1;
  const v = new Int32Array(2 * max + 3);
  const trace: Int32Array[] = [];
  let found = -1;
  for (let d = 0; d <= max && found < 0; d += 1) {
    trace.push(v.slice());
    for (let k = -d; k <= d; k += 2) {
      let x =
        k === -d || (k !== d && v[offset + k - 1] < v[offset + k + 1])
          ? v[offset + k + 1]
          : v[offset + k - 1] + 1;
      let y = x - k;
      while (x < n && y < m && a[aLo + x] === b[bLo + y]) {
        x += 1;
        y += 1;
      }
      v[offset + k] = x;
      if (x >= n && y >= m) {
        found = d;
        break;
      }
    }
  }
  if (found < 0) return [{ aStart: aLo, aEnd: aHi, bStart: bLo, bEnd: bHi }];

  const edits: { x: number; y: number; insert: boolean }[] = [];
  let x = n;
  let y = m;
  for (let d = found; d > 0; d -= 1) {
    const before = trace[d];
    const k = x - y;
    const insert =
      k === -d || (k !== d && before[offset + k - 1] < before[offset + k + 1]);
    const prevK = insert ? k + 1 : k - 1;
    const prevX = before[offset + prevK];
    const prevY = prevX - prevK;
    edits.push({ x: prevX, y: prevY, insert });
    x = prevX;
    y = prevY;
  }
  edits.reverse();

  const hunks: Hunk[] = [];
  for (const edit of edits) {
    const aAt = aLo + edit.x;
    const bAt = bLo + edit.y;
    let hunk = hunks[hunks.length - 1];
    if (!hunk || hunk.aEnd !== aAt || hunk.bEnd !== bAt) {
      hunk = { aStart: aAt, aEnd: aAt, bStart: bAt, bEnd: bAt };
      hunks.push(hunk);
    }
    if (edit.insert) hunk.bEnd += 1;
    else hunk.aEnd += 1;
  }
  return hunks;
}

/** Lignes ajoutées et retirées de A vers B. */
export function lineCounts(hunks: readonly Hunk[]): {
  added: number;
  removed: number;
} {
  let added = 0;
  let removed = 0;
  for (const hunk of hunks) {
    added += hunk.bEnd - hunk.bStart;
    removed += hunk.aEnd - hunk.aStart;
  }
  return { added, removed };
}

/** Index (dans B) des lignes ajoutées ou modifiées, pour la gouttière. */
export function changedLines(hunks: readonly Hunk[]): Set<number> {
  const lines = new Set<number>();
  for (const hunk of hunks) {
    for (let line = hunk.bStart; line < hunk.bEnd; line += 1) lines.add(line);
  }
  return lines;
}

// --- Trois versions --------------------------------------------------------

export type RegionKind = "server" | "yours" | "same" | "mixed";

/** Zone où le serveur, vous ou les deux avez changé la version d'ouverture. */
export interface Region {
  kind: RegionKind;
  /** Lignes `[start, end)` de chaque version. */
  base: [number, number];
  server: [number, number];
  yours: [number, number];
}

/** Décalage cumulé (lignes de B moins lignes de A) des plages `hunks`. */
function shift(hunks: readonly Hunk[]): number {
  return hunks.reduce(
    (total, h) => total + (h.bEnd - h.bStart) - (h.aEnd - h.aStart),
    0,
  );
}

function sameLines(
  a: readonly string[],
  [aStart, aEnd]: [number, number],
  b: readonly string[],
  [bStart, bEnd]: [number, number],
): boolean {
  if (aEnd - aStart !== bEnd - bStart) return false;
  for (let i = 0; i < aEnd - aStart; i += 1) {
    if (a[aStart + i] !== b[bStart + i]) return false;
  }
  return true;
}

/**
 * Régions de changement, dans l'ordre. Les plages du serveur et les vôtres
 * qui se chevauchent ou se touchent (même ligne d'ouverture) forment une
 * seule région, mixte si les deux versions y diffèrent.
 */
export function threeWay(
  base: readonly string[],
  server: readonly string[],
  yours: readonly string[],
): Region[] {
  const fromServer = diffLines(base, server).map((h) => ({ h, side: "s" }));
  const fromYours = diffLines(base, yours).map((h) => ({ h, side: "y" }));
  const all = [...fromServer, ...fromYours].sort(
    (p, q) => p.h.aStart - q.h.aStart || p.h.aEnd - q.h.aEnd,
  );
  const regions: Region[] = [];
  let serverDone: Hunk[] = [];
  let yoursDone: Hunk[] = [];
  let index = 0;
  while (index < all.length) {
    const group = [all[index]];
    let end = all[index].h.aEnd;
    index += 1;
    while (index < all.length && all[index].h.aStart <= end) {
      end = Math.max(end, all[index].h.aEnd);
      group.push(all[index]);
      index += 1;
    }
    const start = group[0].h.aStart;
    const serverHunks = group.filter((g) => g.side === "s").map((g) => g.h);
    const yoursHunks = group.filter((g) => g.side === "y").map((g) => g.h);
    const sBefore = shift(serverDone);
    const yBefore = shift(yoursDone);
    const region: Region = {
      kind: "mixed",
      base: [start, end],
      server: [start + sBefore, end + sBefore + shift(serverHunks)],
      yours: [start + yBefore, end + yBefore + shift(yoursHunks)],
    };
    if (yoursHunks.length === 0) region.kind = "server";
    else if (serverHunks.length === 0) region.kind = "yours";
    else if (sameLines(server, region.server, yours, region.yours)) {
      region.kind = "same";
    }
    regions.push(region);
    serverDone = serverDone.concat(serverHunks);
    yoursDone = yoursDone.concat(yoursHunks);
  }
  return regions;
}
