/**
 * Fonctions pures de l'explorateur : chemins distants (POSIX), validation
 * des noms saisis (mêmes règles que le backend, qui reste juge), tri de la
 * liste et genre affiché d'une entrée.
 */
import type { FileEntry } from "./bindings/FileEntry";

/** Longueur maximale d'un nom, en octets (`NAME_MAX` de Linux). */
const MAX_NAME_BYTES = 255;

/** Chemin d'un élément du dossier `dir`. */
export function joinPath(dir: string, name: string): string {
  return dir.endsWith("/") ? `${dir}${name}` : `${dir}/${name}`;
}

/** Dossier parent ; `null` pour la racine. */
export function parentPath(path: string): string | null {
  if (path === "/") return null;
  const index = path.lastIndexOf("/");
  return index <= 0 ? "/" : path.slice(0, index);
}

/** Composants d'un chemin absolu (`/home/deploy` → `home`, `deploy`). */
export function pathSegments(path: string): string[] {
  return path.split("/").filter((segment) => segment !== "");
}

export function pathFromSegments(segments: readonly string[]): string {
  return `/${segments.join("/")}`;
}

/** Nom d'un fichier local, séparateur Windows ou Unix. */
export function localBaseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Problème d'un nom saisi, ou `null` s'il est valide. */
export function nameProblem(name: string): string | null {
  if (name === "") return "Le nom est vide.";
  if (name === "." || name === "..") {
    return "« . » et « .. » ne sont pas des noms valides.";
  }
  if (name.includes("/")) return "Le nom ne peut pas contenir « / ».";
  if (name.includes("\0")) {
    return "Le nom ne peut pas contenir de caractère nul.";
  }
  if (new TextEncoder().encode(name).length > MAX_NAME_BYTES) {
    return "Le nom dépasse 255 octets.";
  }
  return null;
}

/** Dossier, ou lien vers un dossier : s'ouvre et se range avec les dossiers. */
export function isDirLike(entry: FileEntry): boolean {
  return (
    entry.kind === "dir" ||
    (entry.kind === "symlink" && entry.link?.targetKind === "dir")
  );
}

/** Fichier ordinaire, ou lien vers un fichier : peut être téléchargé. */
export function isFileLike(entry: FileEntry): boolean {
  return (
    entry.kind === "file" ||
    (entry.kind === "symlink" && entry.link?.targetKind === "file")
  );
}

export type SortKey = "name" | "size" | "modified" | "permissions";

export interface SortOrder {
  key: SortKey;
  descending: boolean;
}

const COLLATOR = new Intl.Collator("fr", {
  numeric: true,
  sensitivity: "base",
});

function byName(a: FileEntry, b: FileEntry): number {
  return COLLATOR.compare(a.name, b.name) || (a.name < b.name ? -1 : 1);
}

function byKey(key: SortKey, a: FileEntry, b: FileEntry): number {
  switch (key) {
    case "name":
      return 0;
    case "size":
      return a.size - b.size;
    case "modified":
      return (a.modified ?? 0) - (b.modified ?? 0);
    case "permissions":
      return a.permissions.localeCompare(b.permissions);
  }
}

/**
 * Tri de la liste : dossiers (et liens vers un dossier) toujours en premier,
 * puis la colonne choisie, le nom départageant. Les dossiers n'ont pas de
 * taille affichée : triés par taille, ils restent par nom.
 */
export function sortEntries(
  entries: readonly FileEntry[],
  order: SortOrder,
): FileEntry[] {
  const direction = order.descending ? -1 : 1;
  return [...entries].sort((a, b) => {
    const dirA = isDirLike(a);
    const dirB = isDirLike(b);
    if (dirA !== dirB) return dirA ? -1 : 1;
    const key = order.key === "size" && dirA ? "name" : order.key;
    return direction * (byKey(key, a, b) || byName(a, b));
  });
}
