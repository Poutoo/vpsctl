import { describe, expect, it } from "vitest";
import {
  changedLines,
  diffLines,
  lineCounts,
  splitLines,
  threeWay,
  type Hunk,
} from "./textDiff";

/** Applique les plages à A : doit redonner B. */
function apply(a: string[], b: string[], hunks: Hunk[]): string[] {
  const out: string[] = [];
  let index = 0;
  for (const h of hunks) {
    out.push(...a.slice(index, h.aStart), ...b.slice(h.bStart, h.bEnd));
    index = h.aEnd;
  }
  return [...out, ...a.slice(index)];
}

const lines = (text: string) => splitLines(text);

describe("diffLines", () => {
  it("ne trouve rien entre deux textes égaux", () => {
    expect(diffLines(lines("a\nb\n"), lines("a\nb\n"))).toEqual([]);
  });

  it("repère une modification, un ajout et un retrait", () => {
    const a = lines("a\nb\nc\nd\n");
    const b = lines("a\nB\nc\nd\ne\n");
    const hunks = diffLines(a, b);
    expect(hunks).toEqual([
      { aStart: 1, aEnd: 2, bStart: 1, bEnd: 2 },
      { aStart: 4, aEnd: 4, bStart: 4, bEnd: 5 },
    ]);
    expect(lineCounts(hunks)).toEqual({ added: 2, removed: 1 });
    expect([...changedLines(hunks)]).toEqual([1, 4]);
  });

  it("rend visible le retour final ajouté ou retiré", () => {
    expect(lineCounts(diffLines(lines("a"), lines("a\n")))).toEqual({
      added: 1,
      removed: 0,
    });
  });

  it("reconstruit B quel que soit le mélange de changements", () => {
    const cases: [string, string][] = [
      ["a\nb\nc\nd\ne\nf\n", "x\nb\nc\ny\nz\nf\nw\n"],
      ["1\n2\n3\n", ""],
      ["", "1\n2\n"],
      ["a\na\na\nb\n", "a\nb\na\na\n"],
      ["k=1\nk=2\nk=3\n", "k=3\nk=2\nk=1\n"],
    ];
    for (const [x, y] of cases) {
      const a = lines(x);
      const b = lines(y);
      expect(apply(a, b, diffLines(a, b))).toEqual(b);
    }
  });

  it("garde une différence minimale", () => {
    const a = lines("a\nb\nc\nd\ne\n");
    const b = lines("a\nc\nd\nb\ne\n");
    const { added, removed } = lineCounts(diffLines(a, b));
    expect(added + removed).toBe(2);
  });

  it("reste exacte au-delà de la limite de modifications", () => {
    const a = Array.from({ length: 3000 }, (_, i) => `a${i}`);
    const b = Array.from({ length: 3000 }, (_, i) => `b${i}`);
    expect(apply(a, b, diffLines(a, b))).toEqual(b);
  });
});

describe("threeWay", () => {
  const base = lines("A=1\nB=2\nC=3\nD=4\nE=5\n");

  it("attribue chaque changement à son auteur", () => {
    const server = lines("A=1\nB=server\nC=3\nD=4\nE=5\n");
    const yours = lines("A=1\nB=2\nC=3\nD=4\nE=you\n");
    expect(threeWay(base, server, yours)).toEqual([
      { kind: "server", base: [1, 2], server: [1, 2], yours: [1, 2] },
      { kind: "yours", base: [4, 5], server: [4, 5], yours: [4, 5] },
    ]);
  });

  it("décale les plages selon les changements précédents", () => {
    const server = lines("NEW=0\nA=1\nB=2\nC=3\nD=4\nE=5\n");
    const yours = lines("A=1\nB=2\nC=3\nE=5\n");
    expect(threeWay(base, server, yours)).toEqual([
      { kind: "server", base: [0, 0], server: [0, 1], yours: [0, 0] },
      { kind: "yours", base: [3, 4], server: [4, 5], yours: [3, 3] },
    ]);
  });

  it("signale un bloc mixte, et un changement identique des deux côtés", () => {
    const server = lines("A=1\nB=s\nC=3\nD=4\nE=same\n");
    const yours = lines("A=1\nB=y\nC=3\nD=4\nE=same\n");
    const regions = threeWay(base, server, yours);
    expect(regions.map((r) => r.kind)).toEqual(["mixed", "same"]);
  });

  it("réunit deux changements voisins en un bloc mixte", () => {
    const server = lines("A=1\nB=s\nC=3\nD=4\nE=5\n");
    const yours = lines("A=1\nB=2\nC=y\nD=4\nE=5\n");
    expect(threeWay(base, server, yours)).toEqual([
      { kind: "mixed", base: [1, 3], server: [1, 3], yours: [1, 3] },
    ]);
  });
});
