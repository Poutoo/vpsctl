import type { RunState } from "../../bindings/RunState";

/** Ordre d'affichage des états dans les compteurs. */
export const RUN_STATES: RunState[] = [
  "nominal",
  "stopped",
  "failed",
  "transition",
];

export const RUN_STATE_LABEL: Record<RunState, string> = {
  nominal: "Nominal",
  stopped: "Arrêté",
  failed: "En échec",
  transition: "En transition",
};

/** Forme de chaque état (`StatusIcon`) : un état n'est jamais indiqué par la couleur seule. */
export const RUN_STATE_SHAPE = {
  nominal: "disc",
  stopped: "circle",
  failed: "triangle",
  transition: "half",
} as const satisfies Record<RunState, string>;

const COUNT_LABEL: Record<RunState, [one: string, many: string]> = {
  nominal: ["nominal", "nominaux"],
  stopped: ["arrêté", "arrêtés"],
  failed: ["en échec", "en échec"],
  transition: ["en transition", "en transition"],
};

export interface StateCount {
  state: RunState;
  count: number;
  /** Ex. « 2 nominaux ». */
  label: string;
}

/** Compteurs par état, dans l'ordre d'affichage. Les états absents sont omis. */
export function countsByState(states: RunState[]): StateCount[] {
  return RUN_STATES.flatMap((state) => {
    const count = states.filter((s) => s === state).length;
    if (count === 0) return [];
    const [one, many] = COUNT_LABEL[state];
    return [{ state, count, label: `${count} ${count > 1 ? many : one}` }];
  });
}
