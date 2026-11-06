/**
 * docs/tests-jalon-13.md, section 10 : seuils propres à chaque profil,
 * valeurs par défaut, saisie invalide, conservation après fermeture.
 */
import { $, browser, expect } from "@wdio/globals";
import { OTHER_PROFILE, PROFILE } from "../../support/appConfig.ts";
import {
  alert,
  card,
  goResources,
  resetThresholds,
  seedResources,
  setThreshold,
  thresholdValue,
  waitCpuValue,
} from "../../support/resources.ts";
import { connect, disconnect, restartApp, textOf } from "../../support/ui.ts";

async function thresholds(): Promise<string[]> {
  const values: string[] = [];
  for (const resource of ["disk", "memory", "load"] as const)
    for (const level of ["attention", "critical"] as const)
      values.push(await thresholdValue(resource, level));
  return values;
}

const DEFAULTS = ["80", "90", "85", "95", "1,0", "2,0"];

describe("Jalon 13 · 10. Seuils propres à chaque profil, valeurs par défaut", () => {
  before(() => seedResources());

  it("chaque profil a ses seuils ; l'historique repart d'un profil à l'autre", async () => {
    await connect(PROFILE.name);
    await goResources();
    await waitCpuValue();
    await setThreshold("disk", "attention", "50");
    await setThreshold("disk", "critical", "60");

    await disconnect();
    await connect(OTHER_PROFILE.name);
    await goResources();
    expect(await textOf('[data-testid="thresholds-profile"]')).toBe(
      OTHER_PROFILE.name,
    );
    expect(await thresholds()).toEqual(DEFAULTS);
    // Historique vide : première lecture.
    expect(
      await textOf(`${card("cpu")} [data-testid="chart-message"]`),
    ).toMatch(/^Première lecture/);

    await disconnect();
    await connect(PROFILE.name);
    await goResources();
    await browser.waitUntil(
      async () => (await thresholdValue("disk", "attention")) === "50",
    );
    expect(await thresholdValue("disk", "critical")).toBe("60");
  });

  it("« Valeurs par défaut » ne touche que le profil affiché", async () => {
    await disconnect();
    await connect(OTHER_PROFILE.name);
    await goResources();
    await setThreshold("memory", "attention", "70");
    await disconnect();
    await connect(PROFILE.name);
    await goResources();
    await resetThresholds();
    expect(await thresholds()).toEqual(DEFAULTS);
    await disconnect();
    await connect(OTHER_PROFILE.name);
    await goResources();
    await browser.waitUntil(
      async () => (await thresholdValue("memory", "attention")) === "70",
    );
  });

  it("saisie invalide : raison affichée, rien d'enregistré, alertes inchangées", async () => {
    await resetThresholds();
    for (const [resource, level, value, reason] of [
      ["disk", "attention", "95", /Inférieur au seuil critique/],
      ["disk", "attention", "0", /entier de 1 à 100/],
      ["memory", "critical", "101", /entier de 1 à 100/],
      ["load", "attention", "0", /0,1 à 64/],
      ["load", "attention", "0,05", /0,1 à 64/],
    ] as const) {
      await setThreshold(resource, level, value);
      expect(
        await textOf(`[data-testid="threshold-error-${resource}-${level}"]`),
      ).toMatch(reason);
      expect(await $(alert("disk")).isExisting()).toBe(false);
      await resetThresholds();
    }
  });

  it("un seuil s'applique tout de suite et reste après fermeture d'Atlas", async () => {
    await waitCpuValue();
    await setThreshold("disk", "attention", "1");
    // Sans attendre un nouveau relevé : les niveaux sont recalculés aussitôt.
    await $(alert("disk")).waitForDisplayed({ timeout: 3_000 });
    await restartApp();
    await connect(OTHER_PROFILE.name);
    await goResources();
    await browser.waitUntil(
      async () => (await thresholdValue("disk", "attention")) === "1",
    );
  });
});
