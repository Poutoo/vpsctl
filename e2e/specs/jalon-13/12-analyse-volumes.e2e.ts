/**
 * docs/tests-jalon-13.md, section 12 : analyse des volumes, annulation
 * vérifiée sur le serveur, analyse qui continue hors de l'écran, limite des
 * flux. L'analyse de la cible ne dure qu'une fraction de seconde : pour
 * l'observer en cours, le démon Docker est suspendu (`holdDocker`), ce qui
 * laisse `docker system df -v` en attente sur le serveur.
 */
import { $, browser, expect } from "@wdio/globals";
import {
  goFiles,
  goResources,
  holdDocker,
  processes,
  releaseDocker,
  seedResources,
  setThreshold,
  surveyTime,
  waitCpuValue,
} from "../../support/resources.ts";
import { seed } from "../../support/target.ts";
import {
  connect,
  goDashboard,
  textOf,
  waitTelemetryTab,
} from "../../support/ui.ts";

const ANALYZE = '[data-testid="volumes-analyze"]';
const RUNNING = '[data-testid="volumes-running"]';
const LIST = '[data-testid="volumes-list"]';
const DF_V = "docker system df -v";

describe("Jalon 13 · 12. Analyse des volumes", () => {
  before(async () => {
    // nominal-web-1 et kuma servent à ouvrir quatre flux (dernier test).
    seed();
    seedResources();
    await connect();
    await goResources();
    await waitCpuValue();
    await setThreshold("disk", "attention", "1");
    await $(ANALYZE).waitForDisplayed({ timeout: 60_000 });
  });

  after(() => releaseDocker());

  it("terminée : volumes triés avec leurs conteneurs, Relancer l'analyse", async () => {
    await $(ANALYZE).click();
    await $(LIST).waitForDisplayed({ timeout: 120_000 });
    expect(await textOf('[data-testid="docker-space"]')).toMatch(
      /VOLUMES · ANALYSE DE \d{2}:\d{2}:\d{2} · [\d,]+ (s|min)/i,
    );
    expect(await textOf(LIST)).toMatch(/atlas-gros · atlas-limite/);
    expect(await textOf(ANALYZE)).toBe("Relancer l'analyse");
  });

  it("en cours : chemin, relevés qui continuent, Annuler", async () => {
    holdDocker();
    await $(ANALYZE).click();
    await $(RUNNING).waitForDisplayed();
    expect(await textOf(RUNNING)).toMatch(
      /^ANALYSE EN COURS \/var\/lib\/docker · peut durer plusieurs minutes\. Les relevés continuent\. Annuler$/i,
    );
    const before = await surveyTime();
    await browser.waitUntil(async () => (await surveyTime()) !== before, {
      timeout: 40_000,
      timeoutMsg: "relevés interrompus pendant l'analyse",
    });
  });

  it("Annuler arrête la commande sur le serveur", async () => {
    await browser.waitUntil(() => processes(DF_V).length > 0, {
      timeoutMsg: "analyse absente du serveur",
    });
    await $('[data-testid="volumes-cancel"]').click();
    await $(ANALYZE).waitForDisplayed();
    await browser.waitUntil(() => processes(DF_V).length === 0, {
      timeout: 3_000,
      timeoutMsg: "docker system df -v tourne encore sur le serveur",
    });
    releaseDocker();
  });

  it("quitter l'écran pendant l'analyse ne l'interrompt pas", async () => {
    holdDocker();
    await $(ANALYZE).click();
    await $(RUNNING).waitForDisplayed();
    await goFiles();
    await browser.pause(3_000);
    expect(processes(DF_V).length).toBeGreaterThan(0);
    releaseDocker();
    await goResources();
    await $(LIST).waitForDisplayed({ timeout: 120_000 });
  });

  it("4 flux déjà ouverts : analyse refusée", async () => {
    await goDashboard();
    for (const name of [
      "atlas-limite",
      "atlas-libre",
      "kuma",
      "nominal-web-1",
    ]) {
      await $(`button[aria-label="Télémétrie de ${name}"]`).click();
      await $('button[aria-label="Plein écran"]').click();
      await waitTelemetryTab(name);
      await goDashboard();
    }
    await goResources();
    await $(ANALYZE).click();
    await browser.waitUntil(async () =>
      /4 flux déjà ouverts : fermez un onglet de télémétrie pour lancer l'analyse/.test(
        await textOf('[data-testid="docker-space"]'),
      ),
    );
  });
});
