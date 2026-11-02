/**
 * docs/tests-jalon-11.md, section 12 : rendu sous WebView2. Les formes et
 * les infobulles sont vérifiées par le DOM ; des captures sont enregistrées
 * dans e2e/.cache/screenshots/ pour comparer à l'œil avec les maquettes.
 */
import fs from "node:fs";
import path from "node:path";
import { $, $$, browser, expect } from "@wdio/globals";
import { CACHE_DIR } from "../../support/env.ts";
import { seed, sh } from "../../support/target.ts";
import {
  colorOf,
  connect,
  DOCKER,
  DIALOG,
  noGo,
  projectAction,
  refresh,
  runProjectAction,
  waitDialog,
  waitProjectState,
  waitTelemetryTab,
} from "../../support/ui.ts";

const SHOTS = path.join(CACHE_DIR, "screenshots");

/** Caractères qu'une forme d'état ne doit jamais remplacer. */
const SYMBOLS = /[●○◐◑◒◓◎◉▲△▴▵▼⚠✓✔✗✘✕✖❗❕]/u;

async function screenshot(name: string): Promise<void> {
  fs.mkdirSync(SHOTS, { recursive: true });
  await browser.saveScreenshot(path.join(SHOTS, `${name}.png`));
}

async function shapesIn(scope: string): Promise<string[]> {
  const shapes = Array.from(await $$(`${scope} [data-testid^="shape-"]`));
  return Promise.all(
    shapes.map(
      async (shape) =>
        `${await shape.getTagName()}:${(await shape.getAttribute("data-testid"))?.replace("shape-", "")}`,
    ),
  );
}

describe("Jalon 11 · 12. Rendu", () => {
  before(async () => {
    seed();
    await connect();
  });

  it("tableau de bord : formes en SVG (disque, cible, triangle, cercle), aucun symbole", async () => {
    const shapes = await shapesIn(DOCKER);
    for (const shape of [
      "svg:disc",
      "svg:target",
      "svg:triangle",
      "svg:circle",
    ]) {
      expect(shapes).toContain(shape);
    }
    expect(shapes.every((shape) => shape.startsWith("svg:"))).toBe(true);
    expect(await $(DOCKER).getText()).not.toMatch(SYMBOLS);
    await screenshot("tableau-de-bord-1280");
  });

  it("GO / NO GO : point d'exclamation et coche en SVG, aucun symbole", async () => {
    await runProjectAction("nominal", "Arrêter et supprimer");
    await waitDialog();
    const shapes = await shapesIn(DIALOG);
    expect(shapes).toContain("svg:bang");
    expect(shapes).toContain("svg:check");
    expect(await $(DIALOG).getText()).not.toMatch(SYMBOLS);
    await screenshot("go-no-go-down");
    await noGo();
  });

  it("demi-disque d'une commande en cours en SVG (navigation, en-tête du projet)", async () => {
    sh("cd /tmp && docker compose -p slow down -t 0");
    await refresh();
    await waitProjectState("slow", /^DESCENDU/i);
    await runProjectAction("slow", "Démarrer");
    await waitTelemetryTab("up · slow");
    await $(
      '//nav[@aria-label="Navigation principale"]//a[contains(., "Tableau de bord")]',
    ).click();
    expect(await shapesIn('[data-testid="nav-compose-badge"]')).toEqual([
      "svg:half",
    ]);
    expect(
      await shapesIn(
        '[data-testid="project-slow"] [data-testid="project-state"]',
      ),
    ).toEqual(["svg:half"]);
    await screenshot("up-en-cours-tableau-de-bord");
    await waitProjectState("slow", /^NOMINAL/i, 120_000);
  });

  it("trait de rattachement des conteneurs à l'en-tête de leur groupe", async () => {
    await $(
      '//nav[@aria-label="Navigation principale"]//a[contains(., "Tableau de bord")]',
    ).click();
    const branches = Array.from(
      await $$(
        '[data-testid="container-nominal-web-1"] .branch, [data-testid="container-nominal-app-1"] .branch',
      ),
    );
    expect(branches).toHaveLength(2);
    for (const branch of branches) {
      expect(await colorOf(branch, "border-left-style")).toBe("solid");
      expect(
        parseFloat(await colorOf(branch, "border-left-width")),
      ).toBeGreaterThan(0);
    }
  });

  it("infobulles à deux lignes : libellé en gras au-dessus de l'explication", async () => {
    await projectAction("nominal", "Démarrer").moveTo();
    const tip = $(".app-tooltip");
    await tip.waitForDisplayed();
    const title = await tip.$("strong").getLocation();
    const detail = await tip.$(".app-tooltip-detail").getLocation();
    expect(detail.y).toBeGreaterThan(title.y);
    await screenshot("infobulle-action");
    await $("h1").moveTo();
  });

  it.skip(
    "même rendu sous Linux (WebKitGTK) — hors périmètre : tests E2E sous Windows uniquement",
  );
});
