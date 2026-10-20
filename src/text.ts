/**
 * Opérations sur le texte saisi, identiques à celles du backend : la
 * validation instantanée doit donner le même verdict que Rust.
 */

const EDGE_WHITE_SPACE = /^\p{White_Space}+|\p{White_Space}+$/gu;

/**
 * Retire les espaces de bord comme `str::trim` en Rust (propriété Unicode
 * White_Space). Différent de `String.prototype.trim`, qui retire U+FEFF et
 * garde U+0085.
 */
export function trimLikeRust(value: string): string {
  return value.replace(EDGE_WHITE_SPACE, "");
}

/** Nombre de caractères comme `chars().count()` en Rust (points de code). */
export function charCount(value: string): number {
  return [...value].length;
}
