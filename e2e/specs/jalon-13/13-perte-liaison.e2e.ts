/**
 * docs/tests-jalon-13.md, section 13 : perte de liaison pendant les relevés
 * et pendant l'analyse. La cible est gelée (`docker pause`) au lieu de
 * couper le réseau du poste.
 */
import { $, $$, browser, expect } from "@wdio/globals";
import {
  alert,
  card,
  goResources,
  holdDocker,
  processes,
  releaseDocker,
  seedResources,
  setThreshold,
  SURVEY,
  thresholdValue,
  waitCpuValue,
  waitNextSurvey,
} from "../../support/resources.ts";
import { freezeTarget, thawTarget } from "../../support/target.ts";
import { button, connect, disconnect, textOf } from "../../support/ui.ts";

const DF_V = "docker system df -v";

/** Dégèle la cible si elle est gelée (un test en échec a pu la laisser). */
function safeThaw(): void {
  try {
    thawTarget();
  } catch {
    // Déjà dégelée.
  }
}

async function loseLink(): Promise<void> {
  freezeTarget();
  await button("Rétablir la liaison").waitForDisplayed({ timeout: 60_000 });
}

async function restoreLink(): Promise<void> {
  safeThaw();
  await button("Rétablir la liaison").click();
  await browser.waitUntil(
    async () => !(await button("Rétablir la liaison").isExisting()),
    { timeout: 60_000 },
  );
}

describe("Jalon 13 · 13. Perte de liaison", () => {
  before(async () => {
    seedResources();
    await connect();
    await goResources();
    await waitCpuValue();
    await waitNextSurvey();
  });

  after(() => {
    safeThaw();
    releaseDocker();
  });

  it("pendant les relevés : valeurs figées, zone sans relevé, bandeaux complétés", async () => {
    await setThreshold("disk", "attention", "1");
    await $(alert("disk")).waitForDisplayed();
    await loseLink();
    expect(await textOf(SURVEY)).toMatch(
      /^Valeurs figées · dernier relevé \d{2}:\d{2}:\d{2}/i,
    );
    expect(await textOf(card("cpu"))).toMatch(/Figé à \d{2}:\d{2}:\d{2}/);
    expect(await textOf(alert("disk"))).toMatch(
      /Valeur figée : le serveur ne répond plus\.$/,
    );
    await browser.waitUntil(
      async () =>
        (await $$(`${card("cpu")} [data-testid="chart-gap"]`).length) > 0,
      { timeoutMsg: "aucune zone sans relevé" },
    );
    expect(await $('[data-testid^="nav-resources-"]').isDisplayed()).toBe(true);
  });

  it("seuils modifiables pendant la perte, alertes recalculées", async () => {
    await setThreshold("disk", "attention", "80");
    await browser.waitUntil(async () => !(await $(alert("disk")).isExisting()));
    expect(await thresholdValue("disk", "attention")).toBe("80");
  });

  it("rétablissement : relevés repris, historique et zone sans relevé gardés", async () => {
    await restoreLink();
    await browser.waitUntil(async () =>
      /^Relevé \d{2}:\d{2}:\d{2}/.test(await textOf(SURVEY)),
    );
    expect(
      await $$(`${card("cpu")} [data-testid="chart-gap"]`).length,
    ).toBeGreaterThan(0);
    await waitCpuValue(20_000);
  });

  it("pendant une analyse : à relancer, arrêtée sur le serveur, relançable", async () => {
    await setThreshold("disk", "attention", "1");
    const analyze = $('[data-testid="volumes-analyze"]');
    await analyze.waitForDisplayed({ timeout: 60_000 });
    // Démon suspendu : l'analyse reste en cours sur le serveur (section 12).
    holdDocker();
    await analyze.click();
    await $('[data-testid="volumes-running"]').waitForDisplayed();
    await browser.waitUntil(() => processes(DF_V).length > 0);
    await loseLink();
    await $('[data-testid="volumes-lost"]').waitForDisplayed();
    expect(await textOf('[data-testid="volumes-lost"]')).toBe(
      "Liaison perdue · analyse à relancer",
    );
    expect(await analyze.isEnabled()).toBe(false);

    await restoreLink();
    // Canal fermé avec la liaison : le script de surveillance a arrêté la commande.
    await browser.waitUntil(() => processes(DF_V).length === 0, {
      timeout: 30_000,
      timeoutMsg: "docker system df -v tourne encore sur le serveur",
    });
    releaseDocker();
    await browser.waitUntil(async () => analyze.isEnabled());
    await analyze.click();
    await $('[data-testid="volumes-list"]').waitForDisplayed({
      timeout: 200_000,
    });
  });

  it("« Couper la liaison » vide l'historique ; reconnexion : première lecture", async () => {
    safeThaw();
    releaseDocker();
    await disconnect();
    await connect();
    await goResources();
    expect(
      await textOf(`${card("cpu")} [data-testid="chart-message"]`),
    ).toMatch(/^Première lecture/);
    // Historique vidé : plus de zone sans relevé. Les seuils du profil, eux,
    // sont gardés.
    expect(await $$(`${card("cpu")} [data-testid="chart-gap"]`).length).toBe(0);
    expect(await thresholdValue("disk", "attention")).toBe("1");
  });
});
