/**
 * docs/tests-jalon-12.md, section 12 : Tab insère une tabulation, Échap
 * puis Tab sort de la zone, Ctrl+Z annule. La saisie avec touches mortes
 * reste manuelle.
 */
import { $, browser, expect } from "@wdio/globals";
import {
  EDITOR_DIR,
  STATUS,
  TEXT,
  editorValue,
  openEditor,
  seedEditor,
} from "../../support/editor.ts";
import { connect, textOf } from "../../support/ui.ts";

async function focusInText(): Promise<boolean> {
  return browser.execute(
    () =>
      document.activeElement ===
      document.querySelector('[data-testid="editor-text"]'),
  );
}

describe("Jalon 12 · 12. Clavier", () => {
  before(async () => {
    seedEditor();
    await connect();
    await openEditor(`${EDITOR_DIR}/notes.txt`);
  });

  it("Tab insère une tabulation, le focus reste dans la zone", async () => {
    await $(TEXT).click();
    await browser.keys(["Control", "Home"]);
    await browser.keys("Tab");
    expect(await editorValue()).toBe("\tun\ndeux\ntrois\nquatre\ncinq\n");
    expect(await focusInText()).toBe(true);
  });

  it("Ctrl+Z annule la tabulation", async () => {
    await browser.keys(["Control", "z"]);
    expect(await editorValue()).toBe("un\ndeux\ntrois\nquatre\ncinq\n");
  });

  it("Échap puis Tab sort de la zone", async () => {
    await $(TEXT).click();
    await browser.keys("Escape");
    expect(await textOf(STATUS)).toContain("Tab : quitter la zone");
    await browser.keys("Tab");
    expect(await focusInText()).toBe(false);
    expect(await editorValue()).toBe("un\ndeux\ntrois\nquatre\ncinq\n");
  });

  it("une autre touche entre Échap et Tab annule la sortie", async () => {
    await $(TEXT).click();
    await browser.keys(["Control", "Home"]);
    await browser.keys("Escape");
    await browser.keys("ArrowRight");
    await browser.keys("Tab");
    expect(await focusInText()).toBe(true);
    expect(await editorValue()).toBe("u\tn\ndeux\ntrois\nquatre\ncinq\n");
  });
});
