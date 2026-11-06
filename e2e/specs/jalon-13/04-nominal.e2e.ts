/** docs/tests-jalon-13.md, section 4 : nominal. */
import { $, $$, expect } from "@wdio/globals";
import {
  card,
  DOCKER_MOUNT,
  diskRow,
  displayedDiskPercent,
  goResources,
  seedResources,
  SURVEY,
  targetDiskPercent,
  targetLoadPerCore,
  targetMemoryPercent,
  thresholdValue,
  waitCpuValue,
  waitNextSurvey,
} from "../../support/resources.ts";
import { connect, textOf } from "../../support/ui.ts";

describe("Jalon 13 · 4. Nominal", () => {
  before(async () => {
    seedResources();
    await connect();
    await goResources();
    await waitCpuValue();
  });

  it("en-tête : heure du relevé, toutes les 5 s, 30 dernières minutes", async () => {
    expect(await textOf(SURVEY)).toMatch(
      /^Relevé \d{2}:\d{2}:\d{2} · toutes les 5 s 30 dernières minutes · en mémoire$/i,
    );
    await waitNextSurvey(12_000);
  });

  it("CPU : pourcentage, cœurs, pic sur 30 min et courbe", async () => {
    const value = await waitCpuValue();
    expect(value).toBeGreaterThanOrEqual(0);
    expect(value).toBeLessThanOrEqual(100);
    expect(await textOf(card("cpu"))).toMatch(
      /\d+ cœurs · pic \d+ % sur 30 min/,
    );
    expect(await $(`${card("cpu")} path.line`).getAttribute("d")).toMatch(/^M/);
  });

  it("mémoire calculée sur MemAvailable, swap et seuils 85 / 95 %", async () => {
    const text = await textOf(card("memory"));
    const shown = Number(/sur [\d,]+ [KMGT]?[oi]+ · (\d+) %/.exec(text)?.[1]);
    expect(Math.abs(shown - targetMemoryPercent())).toBeLessThanOrEqual(3);
    expect(text).toMatch(/Swap/);
    expect(text).toMatch(/seuil 85 %/);
    expect(text).toMatch(/seuil 95 %/);
  });

  it("charge par cœur sur 5 min, seuils par cœur", async () => {
    const value = Number(
      (await textOf('[data-testid="load-value"]')).replace(",", "."),
    );
    expect(Math.abs(value - targetLoadPerCore())).toBeLessThanOrEqual(0.1);
    const text = await textOf(card("load"));
    expect(text).toMatch(/1 min [\d,]+ · 15 min [\d,]+/);
    expect(text).toMatch(/seuils 1,0 et 2,0/);
  });

  it("réseau : débits reçu et émis, eth0 sans les interfaces de Docker", async () => {
    await $('[data-testid="network-interfaces"]').waitForDisplayed({
      timeout: 15_000,
    });
    const interfaces = await textOf('[data-testid="network-interfaces"]');
    expect(interfaces).toBe("eth0 · reçu et émis");
    expect(await textOf('[data-testid="network-received"]')).toMatch(
      /[KMG]?o\/s$/,
    );
  });

  it("disque : point de montage de Docker, ni overlay ni fichier monté ni doublon", async () => {
    const rows = await $$('[data-testid^="disk-/"]').map((row) =>
      row.getAttribute("data-testid"),
    );
    expect(rows).toEqual([`disk-${DOCKER_MOUNT}`]);
    expect(await displayedDiskPercent(DOCKER_MOUNT)).toBe(
      targetDiskPercent(DOCKER_MOUNT),
    );
    expect(await textOf(diskRow(DOCKER_MOUNT))).toMatch(
      /^\/var\/lib\/docker \/dev\/\w+ · ext4 \d+ % [\d, ]+ [KMGT]o libres sur [\d, ]+ [KMGT]o NOMINAL$/i,
    );
  });

  it("conteneurs triés par mémoire, limite ou « sans limite »", async () => {
    await $('[data-testid="container-atlas-limite"]').waitForDisplayed({
      timeout: 20_000,
    });
    expect(await textOf('[data-testid="container-atlas-limite"]')).toMatch(
      / \/ 256 Mo$/,
    );
    expect(await textOf('[data-testid="container-atlas-libre"]')).toMatch(
      /· sans limite$/,
    );
    const used = await $$("[data-container-id]").map(async (row) => {
      const text = (await row.getText()).replace(/\s+/g, " ");
      const match = /([\d,]+) ([KMG]?o)( \/| ·)/.exec(text);
      const factor =
        { o: 1, Ko: 1e3, Mo: 1e6, Go: 1e9 }[match?.[2] ?? "o"] ?? 1;
      return Number((match?.[1] ?? "0").replace(",", ".")) * factor;
    });
    expect(used).toEqual([...used].sort((a, b) => b - a));
  });

  it("seuils d'alerte par défaut", async () => {
    expect(await thresholdValue("disk", "attention")).toBe("80");
    expect(await thresholdValue("disk", "critical")).toBe("90");
    expect(await thresholdValue("memory", "attention")).toBe("85");
    expect(await thresholdValue("memory", "critical")).toBe("95");
    expect(await thresholdValue("load", "attention")).toBe("1,0");
    expect(await thresholdValue("load", "critical")).toBe("2,0");
    expect(await textOf('[data-testid="thresholds-profile"]')).toBe(
      "atlas-e2e",
    );
  });
});
