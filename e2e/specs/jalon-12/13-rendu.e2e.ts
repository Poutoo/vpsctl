/**
 * docs/tests-jalon-12.md, section 13 : rendu sous WebView2 à 1280 et
 * 1024 px. Captures dans e2e/.cache/screenshots/ pour comparer à l'œil avec
 * les maquettes ; le rendu sous Linux (WebKitGTK) reste manuel.
 */
import fs from "node:fs";
import path from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  CLOSE,
  EDITOR_DIR,
  SAVE,
  openEditor,
  pressSave,
  replaceLine,
  seedEditor,
} from "../../support/editor.ts";
import { CACHE_DIR } from "../../support/env.ts";
import { sh } from "../../support/target.ts";
import {
  DIALOG,
  connect,
  dismissToasts,
  waitDialog,
} from "../../support/ui.ts";

const SHOTS = path.join(CACHE_DIR, "screenshots");

async function screenshot(name: string): Promise<void> {
  fs.mkdirSync(SHOTS, { recursive: true });
  await browser.saveScreenshot(path.join(SHOTS, `editeur-${name}.png`));
}

/** L'élément tient dans la fenêtre, sans débordement horizontal. */
async function fits(selector: string): Promise<boolean> {
  return browser.execute((s: string) => {
    const element = document.querySelector(s);
    if (!element) return false;
    const box = element.getBoundingClientRect();
    return box.left >= 0 && box.right <= window.innerWidth;
  }, selector);
}

describe("Jalon 12 · 13. Rendu", () => {
  before(async () => {
    seedEditor();
    await connect();
  });

  for (const [width, height] of [
    [1280, 800],
    [1024, 680],
  ] as const) {
    it(`${width} px : édition, conflit, lecture seule et refus`, async () => {
      await browser.setWindowSize(width, height);
      const file = `${EDITOR_DIR}/conflit.env`;
      await openEditor(file);
      await replaceLine(1, "B=vous");
      await screenshot(`${width}-modifie`);
      expect(await fits(SAVE)).toBe(true);

      sh(`sed -i 's/D=4/D=serveur/' ${file}`);
      await pressSave();
      await waitDialog();
      await browser.pause(400);
      await screenshot(`${width}-conflit`);
      expect(await fits(`${DIALOG} [data-testid="confirm-go"]`)).toBe(true);
      await browser.keys("Escape");
      await $(DIALOG).waitForDisplayed({ reverse: true });

      await $(CLOSE).click();
      await waitDialog();
      await screenshot(`${width}-quitter`);
      expect(await fits(`${DIALOG} [data-testid="confirm-go"]`)).toBe(true);
      await $(`${DIALOG} [data-testid="confirm-go"]`).click();
      await $(DIALOG).waitForDisplayed({ reverse: true });

      seedEditor();
      await openEditor(`${EDITOR_DIR}/autre.conf`);
      await screenshot(`${width}-lecture-seule`);
      await openEditor(`${EDITOR_DIR}/cle.pem`);
      await screenshot(`${width}-refus-cle-privee`);
      await dismissToasts();
    });
  }

  it.skip("Linux (WebKitGTK) : manuel, la suite E2E ne tourne que sous Windows", () => {});
});
