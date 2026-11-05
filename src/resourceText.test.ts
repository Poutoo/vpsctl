import { describe, expect, it } from "vitest";
import { parseThreshold } from "./resourceRules";
import {
  formatCpu,
  formatLoad,
  formatPercent,
  formatRate,
  navLevel,
  resourceAlerts,
  worst,
} from "./resourceText";
import { sampleUpdate } from "./testUtils";

describe("formats", () => {
  it("arrondit comme les alertes", () => {
    expect(formatPercent(79.6)).toBe("80 %");
    expect(formatLoad(0.675)).toBe("0,68");
    expect(formatCpu(4.8)).toBe("4,8 %");
    expect(formatCpu(22)).toBe("22 %");
    expect(formatRate(1.8 * 1024 * 1024)).toBe("1,8 Mo/s");
  });

  it("lit une saisie de seuil avec virgule ou point", () => {
    expect(parseThreshold("0,7")).toBe(0.7);
    expect(parseThreshold(" 85 ")).toBe(85);
    expect(parseThreshold("1.5")).toBe(1.5);
    expect(parseThreshold("")).toBeNaN();
    expect(parseThreshold("-1")).toBeNaN();
    expect(parseThreshold("8a")).toBeNaN();
  });
});

describe("bandeaux d'alerte", () => {
  it("disque, mémoire puis charge, avec la cause Docker", () => {
    const alerts = resourceAlerts(
      sampleUpdate({
        levels: {
          disks: [{ mountPoint: "/var/lib/docker", level: "critical" }],
          memory: "attention",
          load: "attention",
        },
      }),
    );
    expect(alerts.map((a) => a.resource)).toEqual(["disk", "memory", "load"]);
    expect(alerts[0].title).toBe(
      "Disque presque plein · /var/lib/docker · 92 %",
    );
    expect(alerts[0].detail).toBe(
      "8,0 Go libres sur 100 Go. Répartition de l'espace Docker ci-dessous.",
    );
    expect(alerts[0].container).toBeNull();
    expect(alerts[1].title).toBe("Mémoire élevée · 88 %");
    expect(alerts[1].detail).toBe(
      "6,9 Go utilisés sur 7,8 Go ; swap entamé (1,2 Go sur 2,0 Go). Plus gros consommateur : postgres-main, 2,4 Go.",
    );
    expect(alerts[1].container?.name).toBe("postgres-main");
    expect(alerts[2].title).toBe("Charge élevée · 0,68 par cœur");
  });

  it("un point de montage hors Docker n'affiche aucune cause", () => {
    const [alert] = resourceAlerts(
      sampleUpdate({
        levels: {
          disks: [{ mountPoint: "/", level: "attention" }],
          memory: "nominal",
          load: "nominal",
        },
      }),
    );
    expect(alert.title).toBe("Disque bientôt plein · / · 41 %");
    expect(alert.detail).toBe("30 Go libres sur 50 Go.");
    expect(alert.container).toBeNull();
  });

  it("sans conteneurs relevés, l'alerte mémoire ne désigne personne", () => {
    const alerts = resourceAlerts(
      sampleUpdate({
        containers: { status: "notRead" },
        levels: { disks: [], memory: "critical", load: "nominal" },
      }),
    );
    expect(alerts[0].title).toBe("Mémoire saturée · 88 %");
    expect(alerts[0].container).toBeNull();
  });

  it("aucune alerte au nominal", () => {
    expect(
      resourceAlerts(
        sampleUpdate({
          levels: { disks: [], memory: "nominal", load: "nominal" },
        }),
      ),
    ).toEqual([]);
    expect(resourceAlerts(null)).toEqual([]);
  });
});

describe("repère de navigation", () => {
  it("disque et mémoire partout, la charge seulement à l'écran", () => {
    const loadOnly = sampleUpdate({
      levels: { disks: [], memory: "nominal", load: "critical" },
    });
    expect(navLevel(loadOnly, true)).toBe("critical");
    expect(navLevel(loadOnly, false)).toBe("nominal");
    expect(navLevel(sampleUpdate(), false)).toBe("critical");
    expect(navLevel(null, true)).toBe("nominal");
  });

  it("pire niveau", () => {
    expect(worst(["nominal", "attention"])).toBe("attention");
    expect(worst([])).toBe("nominal");
  });
});
