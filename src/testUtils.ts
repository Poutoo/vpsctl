import type { LinkLost } from "./bindings/LinkLost";
import type { ResourceUpdate } from "./bindings/ResourceUpdate";
import { DEFAULT_THRESHOLDS } from "./resourceRules";
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

const GO = 1024 ** 3;

/** Relevé de ressources d'exemple (planche VII, maquette 02). */
export function sampleUpdate(
  overrides: Partial<ResourceUpdate> = {},
): ResourceUpdate {
  return {
    profileId: "p1",
    at: 1_000,
    foreground: true,
    cores: 4,
    cpu: { status: "ok", value: 23 },
    memory: {
      status: "ok",
      value: {
        total: 7.8 * GO,
        used: 6.9 * GO,
        percent: 88,
        swap: { total: 2 * GO, used: 1.2 * GO },
      },
    },
    load: {
      status: "ok",
      value: { raw: [3.1, 2.7, 2.2], perCore: [0.775, 0.675, 0.55] },
    },
    network: {
      status: "ok",
      value: { interfaces: ["eth0"], received: 1.8e6, sent: 0.6e6 },
    },
    disks: {
      status: "ok",
      value: [
        {
          mountPoint: "/",
          device: "/dev/sda1",
          fsType: "ext4",
          space: {
            status: "ok",
            value: { total: 50 * GO, free: 30 * GO, percent: 41 },
          },
        },
        {
          mountPoint: "/var/lib/docker",
          device: "/dev/sdb1",
          fsType: "ext4",
          space: {
            status: "ok",
            value: { total: 100 * GO, free: 8 * GO, percent: 92 },
          },
        },
      ],
    },
    containers: {
      status: "ok",
      value: [
        {
          id: "a".repeat(64),
          name: "postgres-main",
          cpuPercent: 22,
          memoryUsed: 2.4 * GO,
          memoryLimit: 3 * GO,
        },
      ],
    },
    dockerMount: "/var/lib/docker",
    dockerUsage: null,
    automaticDockerUsage: true,
    levels: {
      disks: [
        { mountPoint: "/", level: "nominal" },
        { mountPoint: "/var/lib/docker", level: "critical" },
      ],
      memory: "attention",
      load: "nominal",
    },
    thresholds: DEFAULT_THRESHOLDS,
    ...overrides,
  };
}
