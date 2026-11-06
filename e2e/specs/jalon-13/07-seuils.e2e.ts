/**
 * docs/tests-jalon-13.md, section 7 : seuil franchi pour chaque ressource.
 * Les alertes sont provoquées en abaissant les seuils depuis l'interface ;
 * la charge, par des processus actifs dans la cible.
 */
import { $, $$, browser, expect } from "@wdio/globals";
import {
  alert,
  card,
  diskRow,
  DOCKER_MOUNT,
  goResources,
  resetThresholds,
  seedResources,
  setThreshold,
  waitCpuValue,
} from "../../support/resources.ts";
import { sh } from "../../support/target.ts";
import { connect, textOf } from "../../support/ui.ts";

describe("Jalon 13 · 7. Seuil franchi, pour chaque ressource", () => {
  before(async () => {
    seedResources();
    await connect();
    await goResources();
    await waitCpuValue();
  });

  after(() => seedResources());

  it("disque : attention puis critique, sans renvoi vers un conteneur", async () => {
    await setThreshold("disk", "attention", "1");
    await $(alert("disk")).waitForDisplayed();
    expect(await textOf(alert("disk"))).toMatch(
      /^ATTENTION Disque bientôt plein · \/var\/lib\/docker · \d+ %/i,
    );
    expect(await textOf(diskRow(DOCKER_MOUNT))).toMatch(/ATTENTION$/i);

    await setThreshold("disk", "critical", "2");
    await browser.waitUntil(async () =>
      /^CRITIQUE Disque presque plein/i.test(await textOf(alert("disk"))),
    );
    expect(await textOf(diskRow(DOCKER_MOUNT))).toMatch(/CRITIQUE$/i);
    expect(
      await $(
        `${alert("disk")} [data-testid="alert-show-container"]`,
      ).isExisting(),
    ).toBe(false);
  });

  it("mémoire : bandeau, swap, plus gros consommateur et « Voir »", async () => {
    await setThreshold("memory", "attention", "1");
    await $(alert("memory")).waitForDisplayed();
    // Conteneurs relevés : le plus gros consommateur est nommé.
    await browser.waitUntil(async () =>
      /Plus gros consommateur : [\w.-]+, /.test(await textOf(alert("memory"))),
    );
    const text = await textOf(alert("memory"));
    expect(text).toMatch(/^ATTENTION Mémoire élevée · \d+ %/i);
    expect(text).toMatch(/utilisés sur/);
    expect(await textOf(`${card("memory")} header`)).toMatch(/ATTENTION/i);

    const show = $(`${alert("memory")} [data-testid="alert-show-container"]`);
    const name = (await textOf(show)).replace(/^Voir /, "");
    await show.click();
    const row = $(`[data-testid="container-${name}"]`);
    await browser.waitUntil(async () =>
      ((await row.getAttribute("class")) ?? "").includes("highlighted"),
    );
    // Défilement doux : la ligne arrive dans la fenêtre peu après.
    await browser.waitUntil(() => row.isDisplayed({ withinViewport: true }), {
      timeoutMsg: "ligne du conteneur hors de la fenêtre",
    });
  });

  it("charge : bandeau à 0,1 par cœur une fois la charge montée", async () => {
    await setThreshold("load", "attention", "0,1");
    sh("for i in $(seq $(nproc)); do (timeout 200 yes > /dev/null &); done");
    try {
      await $(alert("load")).waitForDisplayed({ timeout: 180_000 });
      expect(await textOf(alert("load"))).toMatch(
        /^ATTENTION Charge élevée · \d,\d\d par cœur/i,
      );
    } finally {
      sh("pkill -x yes || true");
    }
  });

  it("ordre des bandeaux : disque, mémoire, charge ; plus rien aux valeurs par défaut", async () => {
    const order = await $$('[data-testid^="alert-"][role="alert"]').map((a) =>
      a.getAttribute("data-testid"),
    );
    expect(order).toEqual(["alert-disk", "alert-memory", "alert-load"]);
    await resetThresholds();
    await browser.waitUntil(
      async () => (await $$('[role="alert"]').length) === 0,
      { timeout: 30_000, timeoutMsg: "bandeaux encore affichés" },
    );
  });
});
