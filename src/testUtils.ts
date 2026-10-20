import { useNotify } from "./composables/useNotify";

/** Dernière notification affichée. */
export function lastToast() {
  const { items } = useNotify();
  return items[items.length - 1];
}

/** Cas de `tests/shared/` (format décrit dans son README). */
export interface SharedCase {
  input: unknown;
  valid?: boolean;
  expected?: unknown;
  why: string;
}

interface Repeat {
  repeat: string;
  times: number;
  before?: string;
  after?: string;
}

function isRepeat(value: object): value is Repeat {
  return "repeat" in value;
}

function expand(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(expand);
  if (value === null || typeof value !== "object") return value;
  if (isRepeat(value)) {
    return `${value.before ?? ""}${value.repeat.repeat(value.times)}${value.after ?? ""}`;
  }
  return Object.fromEntries(
    Object.entries(value).map(([key, item]) => [key, expand(item)]),
  );
}

export function sharedCases(file: { cases: SharedCase[] }): SharedCase[] {
  return file.cases.map((c) => ({ ...c, input: expand(c.input) }));
}
