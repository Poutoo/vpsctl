import { describe, expect, it } from "vitest";
import {
  areaPath,
  frameAt,
  HISTORY_MS,
  linePath,
  niceMax,
  peak,
  segments,
  spanRects,
  xOf,
  yOf,
} from "./resourceChart";

const NOW = 10_000_000;
const frame = frameAt(NOW, 300, 60, 100);

describe("cadre des courbes", () => {
  it("couvre les 30 dernières minutes, maintenant à droite", () => {
    expect(xOf(NOW, frame)).toBe(300);
    expect(xOf(NOW - HISTORY_MS, frame)).toBe(0);
    expect(xOf(NOW - HISTORY_MS / 2, frame)).toBe(150);
    // Hors du cadre : ramené au bord.
    expect(xOf(NOW - 2 * HISTORY_MS, frame)).toBe(0);
  });

  it("écrête les valeurs au sommet de l'échelle", () => {
    expect(yOf(0, frame)).toBe(60);
    expect(yOf(50, frame)).toBe(30);
    expect(yOf(150, frame)).toBe(0);
  });
});

describe("segments", () => {
  it("interrompt la ligne au-delà de l'écart toléré", () => {
    const points = [
      { at: 0, value: 1 },
      { at: 5_000, value: 2 },
      { at: 70_000, value: 3 },
      { at: 75_000, value: 4 },
    ];
    expect(segments(points, 15_000).map((s) => s.length)).toEqual([2, 2]);
    expect(segments(points, 90_000)).toHaveLength(1);
  });

  it("écarte les points antérieurs au cadre", () => {
    const points = [
      { at: 0, value: 1 },
      { at: 10, value: 2 },
    ];
    expect(segments(points, 1_000, 5)).toEqual([[{ at: 10, value: 2 }]]);
  });
});

describe("tracés", () => {
  const points = [
    { at: NOW - HISTORY_MS / 2, value: 50 },
    { at: NOW, value: 100 },
  ];

  it("trace une ligne et sa surface fermée sur la base", () => {
    expect(linePath(points, frame, HISTORY_MS)).toBe("M150,30L300,0");
    expect(areaPath(points, frame, HISTORY_MS)).toBe(
      "M150,60L150,30L300,0L300,60Z",
    );
  });

  it("un sous-tracé par segment, un point isolé devient un court trait", () => {
    const split = [
      { at: NOW - HISTORY_MS / 2, value: 0 },
      { at: NOW, value: 0 },
    ];
    expect(linePath(split, frame, 1_000)).toBe("M150,60H152M300,60H300");
    expect(areaPath(split, frame, 1_000)).toBe("");
  });

  it("aucun point : aucun tracé", () => {
    expect(linePath([], frame, 1_000)).toBe("");
  });
});

describe("zones sans relevé", () => {
  it("prolonge une perte en cours jusqu'à maintenant", () => {
    expect(
      spanRects([{ from: NOW - HISTORY_MS / 2, to: null }], frame),
    ).toEqual([{ x: 150, width: 150 }]);
  });

  it("ignore une perte sortie du cadre et rogne celle qui en déborde", () => {
    expect(
      spanRects(
        [
          { from: 0, to: 1_000 },
          { from: NOW - 2 * HISTORY_MS, to: NOW - HISTORY_MS / 2 },
        ],
        frame,
      ),
    ).toEqual([{ x: 0, width: 150 }]);
  });
});

describe("échelle", () => {
  it("arrondit le sommet à 1, 2, 2,5 ou 5 × 10ⁿ", () => {
    expect(niceMax([0.4], 1)).toBe(1);
    expect(niceMax([1.3], 1)).toBe(2);
    expect(niceMax([2.2], 1)).toBe(2.5);
    expect(niceMax([3_100_000], 100_000)).toBe(5_000_000);
    expect(niceMax([], 102_400)).toBe(200_000);
  });

  it("pic sur la fenêtre", () => {
    const points = [
      { at: 0, value: 90 },
      { at: 10, value: 29 },
      { at: 20, value: 12 },
    ];
    expect(peak(points, 5)).toBe(29);
    expect(peak([], 0)).toBeNull();
  });
});
