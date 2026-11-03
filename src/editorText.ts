/**
 * Fonctions pures sur le texte de l'éditeur (fins de ligne LF) : position
 * du curseur, nombre de lignes, taille une fois enregistré, remplacement de
 * lignes.
 */
import type { LineEnding } from "./bindings/LineEnding";

/** Index (à partir de 0) de la ligne qui contient la position `offset`. */
export function lineIndexAt(text: string, offset: number): number {
  let line = 0;
  for (
    let i = text.indexOf("\n");
    i >= 0 && i < offset;
    i = text.indexOf("\n", i + 1)
  ) {
    line += 1;
  }
  return line;
}

/** Lignes du fichier, sans compter la ligne vide qui suit le retour final. */
export function lineTotal(text: string): number {
  if (text === "") return 0;
  let count = 1;
  for (let i = text.indexOf("\n"); i >= 0; i = text.indexOf("\n", i + 1)) {
    count += 1;
  }
  return text.endsWith("\n") ? count - 1 : count;
}

const encoder = new TextEncoder();

/** Taille en octets une fois enregistré : UTF-8, CRLF et BOM rétablis. */
export function savedSize(
  text: string,
  bom: boolean,
  lineEnding: LineEnding,
): number {
  let size = encoder.encode(text).length + (bom ? 3 : 0);
  if (lineEnding === "crlf") {
    for (let i = text.indexOf("\n"); i >= 0; i = text.indexOf("\n", i + 1)) {
      size += 1;
    }
  }
  return size;
}

/** Remplacement minimal : caractères `[from, to)` remplacés par `insert`. */
export interface TextChange {
  from: number;
  to: number;
  insert: string;
}

/**
 * Remplace les lignes `[start, end)` par `lines` et renvoie la plus petite
 * modification de texte équivalente, ou `null` si rien ne change.
 */
export function replaceLines(
  text: string,
  start: number,
  end: number,
  lines: readonly string[],
): TextChange | null {
  const all = text.split("\n");
  const next = [...all.slice(0, start), ...lines, ...all.slice(end)].join("\n");
  if (next === text) return null;
  let from = 0;
  while (
    from < text.length &&
    from < next.length &&
    text[from] === next[from]
  ) {
    from += 1;
  }
  let oldEnd = text.length;
  let newEnd = next.length;
  while (
    oldEnd > from &&
    newEnd > from &&
    text[oldEnd - 1] === next[newEnd - 1]
  ) {
    oldEnd -= 1;
    newEnd -= 1;
  }
  return { from, to: oldEnd, insert: next.slice(from, newEnd) };
}
