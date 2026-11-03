/**
 * Vue Différence de l'éditeur (fonctions pures) : lignes affichées, blocs
 * attribués au serveur ou à vous, action de chaque bloc, et résumés des
 * changements (noms de clés pour un .env, numéros de ligne sinon).
 */
import {
  diffLines,
  splitLines,
  threeWay,
  type Hunk,
  type Region,
} from "./textDiff";

export type Who = "server" | "yours";

/** Ligne de la vue. Numéros à partir de 1, dans leur version. */
export type DiffRow =
  | { type: "same"; server: number; yours: number; text: string }
  | { type: "removed"; server: number; text: string; who: Who }
  | { type: "added"; yours: number; text: string; who: Who };

export type ActionKind = "take" | "restore" | "remove" | "takeMixed";

/** Action unique d'un bloc : remplace vos lignes par celles du serveur. */
export interface BlockAction {
  kind: ActionKind;
  label: string;
  /** Vos lignes effacées (bloc mixte), numérotées dans votre version. */
  erases: number[];
  /** Vos lignes `[start, end)` (index à partir de 0) à remplacer. */
  yours: [number, number];
  /** Lignes du serveur qui les remplacent. */
  replacement: string[];
}

export interface DiffBlock {
  kind: "server" | "yours" | "mixed";
  rows: DiffRow[];
  action: BlockAction | null;
}

export type DiffItem =
  | { type: "gap"; count: number }
  | { type: "row"; row: DiffRow }
  | { type: "block"; block: DiffBlock };

/** Lignes identiques gardées autour de chaque bloc. */
const CONTEXT = 2;

/** « 3, 7–9 » : numéros triés, suites regroupées. */
export function lineRanges(lines: readonly number[]): string {
  const sorted = [...new Set(lines)].sort((a, b) => a - b);
  const parts: string[] = [];
  let index = 0;
  while (index < sorted.length) {
    let end = index;
    while (end + 1 < sorted.length && sorted[end + 1] === sorted[end] + 1) {
      end += 1;
    }
    parts.push(
      end === index ? `${sorted[index]}` : `${sorted[index]}–${sorted[end]}`,
    );
    index = end + 1;
  }
  return parts.join(", ");
}

/** Fichier d'environnement : `.env`, `.env.prod`, `settings.env`. */
export function isEnvFile(name: string): boolean {
  return name === ".env" || name.startsWith(".env.") || name.endsWith(".env");
}

const ENV_KEY = /^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_.]*)\s*=/;
const NAME_LENGTH = 40;

/** Nom d'une ligne de .env : sa clé, sinon son texte (commentaire). */
export function envName(line: string): string | null {
  const key = ENV_KEY.exec(line);
  if (key) return key[1];
  const text = line.trim();
  if (text === "") return null;
  return text.length > NAME_LENGTH ? `${text.slice(0, NAME_LENGTH)}…` : text;
}

const MAX_NAMES = 4;

/** Noms distincts, les premiers seulement. */
export function nameList(lines: readonly string[]): string {
  const names = [
    ...new Set(lines.map(envName).filter((n): n is string => n !== null)),
  ];
  const shown = names.slice(0, MAX_NAMES).join(" · ");
  return names.length > MAX_NAMES
    ? `${shown} · +${names.length - MAX_NAMES}`
    : shown;
}

function lineWord(lines: readonly number[]): string {
  return new Set(lines).size > 1 ? "lignes" : "ligne";
}

/** Lignes d'une plage hors des plages modifiées : inchangées depuis l'ouverture. */
function unchanged(length: number, hunks: readonly Hunk[]): boolean[] {
  const kept = new Array<boolean>(length).fill(true);
  for (const h of hunks) {
    for (let i = h.bStart; i < h.bEnd; i += 1) kept[i] = false;
  }
  return kept;
}

function blockRows(
  region: Region,
  base: readonly string[],
  server: readonly string[],
  yours: readonly string[],
): DiffRow[] {
  const baseSlice = base.slice(...region.base);
  const serverSlice = server.slice(...region.server);
  const yoursSlice = yours.slice(...region.yours);
  const serverKept = unchanged(
    serverSlice.length,
    diffLines(baseSlice, serverSlice),
  );
  const yoursKept = unchanged(
    yoursSlice.length,
    diffLines(baseSlice, yoursSlice),
  );
  const rows: DiffRow[] = [];
  let s = 0;
  let y = 0;
  const same = (until: number) => {
    while (s < until) {
      rows.push({
        type: "same",
        server: region.server[0] + s + 1,
        yours: region.yours[0] + y + 1,
        text: serverSlice[s],
      });
      s += 1;
      y += 1;
    }
  };
  for (const h of diffLines(serverSlice, yoursSlice)) {
    same(h.aStart);
    for (; s < h.aEnd; s += 1) {
      rows.push({
        type: "removed",
        server: region.server[0] + s + 1,
        text: serverSlice[s],
        // Ligne d'ouverture absente de votre version : c'est vous qui l'avez retirée.
        who: serverKept[s] ? "yours" : "server",
      });
    }
    for (; y < h.bEnd; y += 1) {
      rows.push({
        type: "added",
        yours: region.yours[0] + y + 1,
        text: yoursSlice[y],
        // Ligne d'ouverture absente du serveur : c'est le serveur qui l'a retirée.
        who: yoursKept[y] ? "server" : "yours",
      });
    }
  }
  same(serverSlice.length);
  return rows;
}

function blockAction(
  region: Region,
  rows: readonly DiffRow[],
  server: readonly string[],
): BlockAction | null {
  if (region.kind === "yours" || region.kind === "same") return null;
  const replacement = server.slice(...region.server);
  const base = { yours: region.yours, replacement, erases: [] as number[] };
  if (region.kind === "server") {
    if (region.server[0] === region.server[1]) {
      return { ...base, kind: "remove", label: "Retirer" };
    }
    if (region.yours[0] === region.yours[1]) {
      return { ...base, kind: "restore", label: "Reprendre" };
    }
    return { ...base, kind: "take", label: "Prendre la version du serveur" };
  }
  const erases = rows.flatMap((row) =>
    row.type === "added" && row.who === "yours" ? [row.yours] : [],
  );
  const label =
    erases.length === 0
      ? "Prendre la version du serveur · rétablit les lignes que vous aviez retirées"
      : `Prendre la version du serveur · efface ${erases.length > 1 ? "vos" : "votre"} ${lineWord(erases)} ${lineRanges(erases)}`;
  return { ...base, kind: "takeMixed", label, erases };
}

export interface DiffView {
  items: DiffItem[];
  blocks: DiffBlock[];
}

/** Vue Différence SERVEUR → VOUS. */
export function buildDiff(
  baseText: string,
  serverText: string,
  yoursText: string,
): DiffView {
  const base = splitLines(baseText);
  const server = splitLines(serverText);
  const yours = splitLines(yoursText);
  const regions = threeWay(base, server, yours).filter(
    (r) => r.kind !== "same",
  );
  const items: DiffItem[] = [];
  const blocks: DiffBlock[] = [];
  // Lignes identiques entre deux blocs : contexte autour, le reste replié.
  const between = (
    sFrom: number,
    sTo: number,
    yFrom: number,
    first: boolean,
    last: boolean,
  ) => {
    const count = sTo - sFrom;
    const lead = first ? 0 : Math.min(CONTEXT, count);
    const trail = last ? 0 : Math.min(CONTEXT, count - lead);
    const push = (i: number) =>
      items.push({
        type: "row",
        row: {
          type: "same",
          server: sFrom + i + 1,
          yours: yFrom + i + 1,
          text: server[sFrom + i],
        },
      });
    for (let i = 0; i < lead; i += 1) push(i);
    if (count - lead - trail > 0) {
      items.push({ type: "gap", count: count - lead - trail });
    }
    for (let i = count - trail; i < count; i += 1) push(i);
  };
  let s = 0;
  let y = 0;
  regions.forEach((region, index) => {
    between(s, region.server[0], y, index === 0, false);
    const rows = blockRows(region, base, server, yours);
    const block: DiffBlock = {
      kind: region.kind as DiffBlock["kind"],
      rows,
      action: blockAction(region, rows, server),
    };
    blocks.push(block);
    items.push({ type: "block", block });
    s = region.server[1];
    y = region.yours[1];
  });
  between(s, server.length, y, regions.length === 0, true);
  return { items, blocks };
}

/** Ce qu'un choix effacerait : nombre de lignes et résumé. */
export interface LossSummary {
  lines: number;
  detail: string;
}

function summarize(
  rows: readonly DiffRow[],
  env: boolean,
  side: "server" | "yours",
  lines: number,
): LossSummary {
  if (env) {
    return { lines, detail: nameList(rows.map((r) => r.text)) };
  }
  const numbers = rows.flatMap((row) =>
    side === "server"
      ? row.type === "removed"
        ? [row.server]
        : []
      : row.type === "added"
        ? [row.yours]
        : [],
  );
  if (numbers.length > 0) {
    return {
      lines,
      detail: `${lineWord(numbers)} ${lineRanges(numbers)}${side === "server" ? " du serveur" : ""}`,
    };
  }
  const others = rows.length;
  const parts: string[] = [];
  if (others > 0) {
    parts.push(
      side === "server"
        ? `${others} ${others > 1 ? "lignes supprimées" : "ligne supprimée"} sur le serveur`
        : `${others} ${others > 1 ? "lignes retirées" : "ligne retirée"}`,
    );
  }
  return { lines, detail: parts.join(" · ") };
}

/** Changements du serveur perdus en écrasant, et les vôtres en rechargeant. */
export function conflictLosses(
  view: DiffView,
  name: string,
): { overwrite: LossSummary; reload: LossSummary } {
  const env = isEnvFile(name);
  const loss = (who: Who): LossSummary => {
    const rows: DiffRow[] = [];
    // Une ligne modifiée compte une fois : ses deux versions s'affichent.
    let lines = 0;
    for (const block of view.blocks) {
      const mine = block.rows.filter((r) => r.type !== "same" && r.who === who);
      rows.push(...mine);
      lines += Math.max(
        mine.filter((r) => r.type === "removed").length,
        mine.filter((r) => r.type === "added").length,
      );
    }
    return summarize(rows, env, who, lines);
  };
  return { overwrite: loss("server"), reload: loss("yours") };
}

/** Vos modifications depuis l'ouverture : « +2 −1 » et leur résumé. */
export function editSummary(
  baseText: string,
  yoursText: string,
  name: string,
): { added: number; removed: number; detail: string } {
  const base = splitLines(baseText);
  const yours = splitLines(yoursText);
  const hunks = diffLines(base, yours);
  const rows: DiffRow[] = [];
  let added = 0;
  let removed = 0;
  for (const h of hunks) {
    for (let i = h.aStart; i < h.aEnd; i += 1) {
      rows.push({
        type: "removed",
        server: i + 1,
        text: base[i],
        who: "yours",
      });
      removed += 1;
    }
    for (let i = h.bStart; i < h.bEnd; i += 1) {
      rows.push({ type: "added", yours: i + 1, text: yours[i], who: "yours" });
      added += 1;
    }
  }
  return {
    added,
    removed,
    detail: summarize(rows, isEnvFile(name), "yours", rows.length).detail,
  };
}
