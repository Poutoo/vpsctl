/**
 * docs/tests-jalon-13.md, section 15 : rendu sous WebView2 à 1280 et
 * 1024 px. Captures dans e2e/.cache/screenshots/ pour comparer à l'œil avec
 * les maquettes ; le rendu sous Linux (WebKitGTK) reste manuel.
 */
import fs from "node:fs";
import path from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { CACHE_DIR } from "../../support/env.ts";
import {
  card,
  goResources,
  seedResources,
  setThreshold,
  waitCpuValue,
} from "../../support/resources.ts";
import { connect } from "../../support/ui.ts";

const SHOTS = path.join(CACHE_DIR, "screenshots");

async function screenshot(name: string): Promise<void> {
  fs.mkdirSync(SHOTS, { recursive: true });
  await browser.saveScreenshot(path.join(SHOTS, `ressources-${name}.png`));
}

/** Cartes sur une même ligne (même haut). */
async function cardsPerRow(): Promise<number> {
  const tops = await browser.execute(
    (selectors: string[]) =>
      selectors.map(
        (s) => document.querySelector(s)?.getBoundingClientRect().top ?? -1,
      ),
    (["cpu", "memory", "load", "network"] as const).map(card),
  );
  return tops.filter((top) => top === tops[0]).length;
}

async function noHorizontalScroll(): Promise<boolean> {
  return browser.execute(() => {
    const main = document.querySelector("main");
    return !!main && main.scrollWidth <= main.clientWidth;
  });
}

describe("Jalon 13 · 15. Rendu", () => {
  before(async () => {
    seedResources();
    await connect();
    await goResources();
    await waitCpuValue();
  });

  after(async () => {
    await browser.setWindowSize(1280, 800);
  });

  for (const [width, height, perRow] of [
    [1280, 800, 4],
    [1024, 680, 2],
  ] as const) {
    it(`${width} px : cartes ${perRow} par ligne, sans défilement horizontal`, async () => {
      await browser.setWindowSize(width, height);
      await browser.execute(() =>
        document.querySelector("main")?.scrollTo(0, 0),
      );
      await browser.pause(500);
      expect(await cardsPerRow()).toBe(perRow);
      expect(await noHorizontalScroll()).toBe(true);
      await screenshot(`${width}-nominal`);

      await setThreshold("disk", "attention", "1");
      await setThreshold("memory", "attention", "1");
      await $('[data-testid="docker-space"]').waitForDisplayed();
      await browser.execute(() =>
        document.querySelector("main")?.scrollTo(0, 0),
      );
      await screenshot(`${width}-seuil-franchi`);
      expect(await noHorizontalScroll()).toBe(true);
      await setThreshold("disk", "attention", "80");
      await setThreshold("memory", "attention", "85");
    });
  }

  it.skip(
    "Linux (WebKitGTK) : mêmes écrans — manuel : tests E2E sous Windows uniquement",
  );
});
