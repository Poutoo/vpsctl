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

const COUNT_LABEL: Record<RunState, [one: string, many: string]> = {
  nominal: ["nominal", "nominaux"],
  stopped: ["arrêté", "arrêtés"],
  failed: ["en échec", "en échec"],
  transition: ["en transition", "en transition"],
};

/** Compteurs par état, ex. « 2 nominaux · 1 arrêté ». Les états absents sont omis. */
export function formatCounts(states: RunState[]): string {
  return RUN_STATES.map((state) => {
    const count = states.filter((s) => s === state).length;
    if (count === 0) return null;
    const [one, many] = COUNT_LABEL[state];
    return `${count} ${count > 1 ? many : one}`;
  })
    .filter(Boolean)
    .join(" · ");
}
