import { describe, expect, it } from "vitest";
import {
  formatAgo,
  formatBytes,
  formatClock,
  formatDuration,
  formatModified,
} from "./format";

describe("formatBytes", () => {
  it("garde les octets sans décimale", () => {
    expect(formatBytes(0)).toBe("0 o");
    expect(formatBytes(418)).toBe("418 o");
  });

  it("passe à l'unité supérieure en base 1024", () => {
    expect(formatBytes(1024)).toBe("1,0 Ko");
    expect(formatBytes(1229)).toBe("1,2 Ko");
    expect(formatBytes(128 * 1024 * 1024)).toBe("128 Mo");
  });

  it("décide de l'unité sur la valeur arrondie", () => {
    expect(formatBytes(1023.96 * 1024)).toBe("1,0 Mo");
    expect(formatBytes(9.96 * 1024)).toBe("10 Ko");
  });

  it("garde toujours une décimale en mode fixe", () => {
    expect(formatBytes(18 * 1024 * 1024, true)).toBe("18,0 Mo");
    expect(formatBytes(512, true)).toBe("512 o");
  });
});

describe("formatClock", () => {
  it("affiche minutes et secondes sur deux chiffres", () => {
    expect(formatClock(14)).toBe("00:14");
    expect(formatClock(72 * 60 + 5)).toBe("72:05");
  });

  it("arrondit et ignore les valeurs négatives", () => {
    expect(formatClock(59.6)).toBe("01:00");
    expect(formatClock(-3)).toBe("00:00");
  });
});

describe("formatDuration", () => {
  it("donne des secondes avec une décimale sous la minute", () => {
    expect(formatDuration(18_400)).toBe("18,4 s");
  });

  it("donne minutes et secondes, puis heures et minutes", () => {
    expect(formatDuration(125_000)).toBe("2 min 05 s");
    expect(formatDuration(3_780_000)).toBe("1 h 03 min");
  });
});

describe("formatAgo", () => {
  const now = new Date(2026, 9, 20, 12, 0, 0).getTime();

  it("arrondit à l'unité la plus grande", () => {
    expect(formatAgo(now - 30_000, now)).toBe("à l'instant");
    expect(formatAgo(now - 5 * 60_000, now)).toBe("il y a 5 min");
    expect(formatAgo(now - 9 * 3_600_000, now)).toBe("il y a 9 h");
    expect(formatAgo(now - 2 * 86_400_000, now)).toBe("il y a 2 j");
  });

  it("donne la date au-delà de 30 jours", () => {
    expect(formatAgo(now - 40 * 86_400_000, now)).toMatch(/^le 10 sept\./);
  });
});

describe("formatModified", () => {
  const now = new Date(2026, 9, 20);

  it("donne l'heure pour l'année en cours", () => {
    expect(formatModified(new Date(2026, 9, 12, 14, 2).getTime(), now)).toBe(
      "12 oct. 14:02",
    );
  });

  it("donne l'année pour une autre année", () => {
    expect(formatModified(new Date(2025, 9, 12).getTime(), now)).toBe(
      "12 oct. 2025",
    );
  });
});
