import type { LinkLost } from "./bindings/LinkLost";
import { useNotify } from "./composables/useNotify";
import { useConnectionStore } from "./stores/connection";

/** Liaison établie sur le profil `p1`, sans passer par le backend. */
export function linked() {
  const connection = useConnectionStore();
  connection.info = { profileId: "p1", connectedAt: 1, latencyMs: 40 };
  connection.phase = "connected";
  connection.profileId = "p1";
  return connection;
}

export function lossEvent(overrides: Partial<LinkLost> = {}): LinkLost {
  return {
    profileId: "p1",
    cause: "timeout",
    detail: null,
    lastReceivedAt: 1_000,
    detectedAt: 2_000,
    ...overrides,
  };
}

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
