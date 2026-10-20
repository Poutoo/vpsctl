/**
 * Filtre des listes, purement côté interface : sous-chaîne recherchée sans
 * tenir compte de la casse ni des accents (« éte » trouve « Été »).
 */

/** Plage du texte d'origine, en indices UTF-16 (`slice(start, end)`). */
export interface MatchRange {
  start: number;
  end: number;
}

/** Morceau de texte, surligné ou non. */
export interface Segment {
  text: string;
  match: boolean;
}

const MARKS = /\p{M}/gu;

/** Texte comparé : décomposé, sans marques diacritiques, en minuscules. */
export function fold(text: string): string {
  return text.normalize("NFD").toLowerCase().replace(MARKS, "");
}

/** Recherche saisie, prête à comparer ; vide si le filtre est inactif. */
export function foldQuery(query: string): string {
  return fold(query.trim());
}

/**
 * Plages du texte d'origine qui correspondent à la recherche, sans
 * chevauchement. Chaque caractère est replié séparément pour garder la
 * correspondance entre le texte replié et le texte d'origine (un caractère
 * peut donner zéro, un ou plusieurs caractères repliés).
 */
export function matchRanges(text: string, query: string): MatchRange[] {
  const needle = foldQuery(query);
  if (needle === "") return [];

  let folded = "";
  /** Pour chaque unité de `folded`, le caractère d'origine dont elle vient. */
  const origin: MatchRange[] = [];
  let index = 0;
  for (const char of text) {
    const part = fold(char);
    const end = index + char.length;
    const previous = origin[origin.length - 1];
    if (part === "" && previous) {
      // Accent combinant : il appartient à la lettre qui le précède.
      previous.end = end;
    } else {
      const range = { start: index, end };
      folded += part;
      for (let i = 0; i < part.length; i++) origin.push(range);
    }
    index = end;
  }

  const ranges: MatchRange[] = [];
  let from = folded.indexOf(needle);
  while (from !== -1) {
    const start = origin[from].start;
    const end = origin[from + needle.length - 1].end;
    const previous = ranges[ranges.length - 1];
    if (previous && start <= previous.end) previous.end = end;
    else ranges.push({ start, end });
    from = folded.indexOf(needle, from + needle.length);
  }
  return ranges;
}

/** Le texte découpé en morceaux surlignés ou non. */
export function segments(text: string, query: string): Segment[] {
  const ranges = matchRanges(text, query);
  if (ranges.length === 0) return [{ text, match: false }];
  const parts: Segment[] = [];
  let index = 0;
  for (const { start, end } of ranges) {
    if (start > index)
      parts.push({ text: text.slice(index, start), match: false });
    parts.push({ text: text.slice(start, end), match: true });
    index = end;
  }
  if (index < text.length)
    parts.push({ text: text.slice(index), match: false });
  return parts;
}

/** Au moins un des champs contient la recherche ; vrai si elle est vide. */
export function matchesFilter(
  fields: readonly (string | null)[],
  query: string,
): boolean {
  const needle = foldQuery(query);
  if (needle === "") return true;
  return fields.some((field) => field !== null && fold(field).includes(needle));
}

/** Éléments dont un des champs contient la recherche, dans le même ordre. */
export function filterItems<T>(
  items: readonly T[],
  query: string,
  fields: (item: T) => readonly (string | null)[],
): T[] {
  if (foldQuery(query) === "") return [...items];
  return items.filter((item) => matchesFilter(fields(item), query));
}
