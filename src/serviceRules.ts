/**
 * Validation instantanée du formulaire de création de service : mêmes règles
 * que le backend (`src-tauri/src/systemd/unit.rs`, détaillées dans
 * docs/03-architecture.md, « Fichiers d'unité systemd »). Le backend reste
 * l'autorité : il revalide tout, et lui seul produit le texte du fichier.
 *
 * Chaque fonction renvoie le problème à afficher sous le champ, ou `null`.
 */

import { charCount, trimLikeRust } from "./text";

export const MAX_NAME_CHARS = 64;
export const MAX_DESCRIPTION_CHARS = 256;
/** Commande, dossier et valeur d'une variable, en octets. */
export const MAX_VALUE_BYTES = 4096;
export const MAX_ENV_VARS = 32;
export const MAX_ENV_KEY_CHARS = 128;

const NAME = /^[a-z0-9][a-z0-9-]*$/;
const ENV_KEY = /^[A-Za-z_][A-Za-z0-9_]*$/;
/** Caractères de contrôle (catégorie Cc, comme `char::is_control` en Rust). */
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;

const SINGLE_LINE = "Retour à la ligne et caractères de contrôle interdits.";
const TRAILING_BACKSLASH = "Ne doit pas se terminer par « \\ ».";

function bytes(value: string): number {
  return new TextEncoder().encode(value).length;
}

function tooLong(value: string): string | null {
  return bytes(value) > MAX_VALUE_BYTES
    ? `${MAX_VALUE_BYTES} octets au plus.`
    : null;
}

/** Nom saisi sans `.service`. */
export function nameProblem(name: string): string | null {
  if (name === "") return "Le nom de l'unité est requis.";
  if (name.endsWith(".service")) return "Saisissez le nom sans « .service ».";
  if (!NAME.test(name)) {
    return "Minuscules, chiffres et tirets, sans tiret au début.";
  }
  if (charCount(name) > MAX_NAME_CHARS) {
    return `${MAX_NAME_CHARS} caractères au plus.`;
  }
  return null;
}

/** Description facultative. */
export function descriptionProblem(description: string): string | null {
  const value = trimLikeRust(description);
  if (value === "") return null;
  if (CONTROL.test(value)) return SINGLE_LINE;
  if (charCount(value) > MAX_DESCRIPTION_CHARS) {
    return `${MAX_DESCRIPTION_CHARS} caractères au plus.`;
  }
  if (value.endsWith("\\")) return TRAILING_BACKSLASH;
  return null;
}

/** Dossier de travail : `~`, `~/chemin` ou chemin absolu. */
export function workingDirProblem(dir: string): string | null {
  const value = trimLikeRust(dir);
  if (value === "") return "Requis (~ pour le dossier personnel).";
  if (CONTROL.test(value)) return SINGLE_LINE;
  const long = tooLong(value);
  if (long) return long;
  if (value.includes("\\")) return "« \\ » interdit.";
  if (value === "~") return null;
  let rest: string;
  if (value.startsWith("~/")) rest = value.slice(2);
  else if (value.startsWith("/")) rest = value.slice(1);
  else return "Chemin absolu, ~ ou chemin commençant par ~/.";
  if (rest.split("/").some((c) => c === "." || c === "..")) {
    return "Composants « . » et « .. » interdits.";
  }
  return null;
}

interface Word {
  /** Texte tel que saisi. */
  raw: string;
  /** Texte après retrait des guillemets ; `\x` donne `x`. */
  text: string;
}

/**
 * Découpe la commande en mots comme systemd : espaces hors guillemets,
 * `'…'` et `"…"` n'importe où dans un mot, `\` qui protège le caractère
 * suivant (y compris entre guillemets). Renvoie le problème rencontré sous
 * forme de chaîne.
 */
function splitWords(command: string): Word[] | string {
  const words: Word[] = [];
  const chars = [...command];
  let i = 0;
  while (i < chars.length) {
    if (chars[i] === " ") {
      i++;
      continue;
    }
    const start = i;
    let text = "";
    let quote: string | null = null;
    for (; i < chars.length; i++) {
      const c = chars[i];
      if (c === "\\") {
        if (i + 1 >= chars.length) return TRAILING_BACKSLASH;
        text += chars[++i];
      } else if (quote !== null) {
        if (c === quote) quote = null;
        else text += c;
      } else if (c === "'" || c === '"') {
        quote = c;
      } else if (c === " ") {
        break;
      } else {
        text += c;
      }
    }
    if (quote !== null) return "Guillemet non fermé.";
    words.push({ raw: chars.slice(start, i).join(""), text });
  }
  return words;
}

/** Préfixes spéciaux de systemd en tête du programme, refusés. */
const PREFIXES = new Set(["-", "@", ":", "+", "!", "|", ";"]);

/** Commande (`ExecStart=`). */
export function execStartProblem(command: string): string | null {
  const value = trimLikeRust(command);
  if (value === "") return "La commande est requise.";
  if (CONTROL.test(value)) return SINGLE_LINE;
  const long = tooLong(value);
  if (long) return long;
  if (value.endsWith("\\")) return TRAILING_BACKSLASH;
  const words = splitWords(value);
  if (typeof words === "string") return words;
  const program = words[0];
  if (program.raw.includes("\\")) return "« \\ » interdit dans le programme.";
  const first = program.text.charAt(0);
  if (first === "") return "Programme vide.";
  if (first === "~") {
    return "systemd ne développe pas ~ : chemin absolu du programme requis.";
  }
  if (PREFIXES.has(first)) {
    return `Le préfixe spécial « ${first} » de systemd n'est pas pris en charge : commencez par le programme.`;
  }
  if (program.text.includes("/") && !program.text.startsWith("/")) {
    return "Le programme est un chemin absolu ou un simple nom.";
  }
  if (words.slice(1).some((w) => w.raw === ";")) {
    return "Un « ; » isolé sépare deux commandes pour systemd : écrivez \\; ou passez par /bin/sh -c '…'.";
  }
  return null;
}

/** Ligne de variable d'environnement saisie. */
export interface EnvRow {
  key: string;
  value: string;
}

export interface EnvProblems {
  /** Problème de la liste entière (nombre de variables). */
  list: string | null;
  /** Par ligne, dans l'ordre : problème de la clé et de la valeur. */
  rows: { key: string | null; value: string | null }[];
}

/** Ligne laissée vide : ignorée, ni envoyée ni signalée. */
export function isBlankRow(row: EnvRow): boolean {
  return row.key === "" && row.value === "";
}

export function envProblems(rows: EnvRow[]): EnvProblems {
  const filled = rows.filter((row) => !isBlankRow(row));
  const seen = new Set<string>();
  return {
    list:
      filled.length > MAX_ENV_VARS
        ? `${MAX_ENV_VARS} variables au plus.`
        : null,
    rows: rows.map((row) => {
      if (isBlankRow(row)) return { key: null, value: null };
      let key: string | null = null;
      if (row.key === "") key = "Clé requise.";
      else if (!ENV_KEY.test(row.key)) {
        key = "Lettres, chiffres et _, sans chiffre au début.";
      } else if (charCount(row.key) > MAX_ENV_KEY_CHARS) {
        key = `${MAX_ENV_KEY_CHARS} caractères au plus.`;
      } else if (seen.has(row.key)) key = "Clé en double.";
      seen.add(row.key);
      const value = CONTROL.test(row.value) ? SINGLE_LINE : tooLong(row.value);
      return { key, value };
    }),
  };
}
