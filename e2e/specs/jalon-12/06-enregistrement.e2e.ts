/**
 * docs/tests-jalon-12.md, section 6 : « aucune modification », marques des
 * lignes modifiées, Ctrl+S, « identique au serveur » après vérification,
 * notification. L'absence d'interception de Ctrl+S par le moteur web reste
 * un test manuel : WebDriver injecte les touches dans la page.
 */
import { $, browser, expect } from "@wdio/globals";
import {
  BAR_DETAIL,
  EDITOR_DIR,
  SAVE,
  TEXT,
  barText,
  changedNumbers,
  contentOf,
  openEditor,
  pressSave,
  replaceLine,
  seedEditor,
  waitBar,
} from "../../support/editor.ts";
import { connect, dismissToasts, textOf, waitToast } from "../../support/ui.ts";

describe("Jalon 12 · 6. Enregistrement", () => {
  before(async () => {
    seedEditor();
    await connect();
  });

  afterEach(async () => {
    await dismissToasts();
  });

  it("« aucune modification » tant que le texte est celui de l'ouverture", async () => {
    await openEditor(`${EDITOR_DIR}/notes.txt`);
    expect(await $(SAVE).isEnabled()).toBe(false);
    await $(TEXT).click();
    await browser.keys(["Control", "End"]);
    await browser.keys("x");
    await waitBar(/^MODIFIÉ/i);
    await browser.keys("Backspace");
    await waitBar(/^ÉDITION/i);
    expect(await textOf(BAR_DETAIL)).toMatch(/^aucune modification/);
    expect(await $('[data-testid="editor-unsaved"]').isExisting()).toBe(false);
  });

  it("lignes modifiées marquées, compteurs, NON ENREGISTRÉ", async () => {
    await openEditor(`${EDITOR_DIR}/notes.txt`);
    await replaceLine(1, "DEUX");
    await waitBar(/MODIFIÉ · \+1 −1 LIGNES/i);
    expect(await changedNumbers()).toEqual(["2"]);
    expect(await $('[data-testid="editor-unsaved"]').isDisplayed()).toBe(true);
    expect(await textOf(BAR_DETAIL)).toMatch(/^Ctrl\+S enregistre · ouvert à/);
    expect(await $(SAVE).isEnabled()).toBe(true);
  });

  it("Ctrl+S enregistre : identique au serveur, notification", async () => {
    await pressSave();
    await waitBar(/^✓? ?ENREGISTRÉ À \d\d:\d\d:\d\d/i);
    expect(await textOf(BAR_DETAIL)).toBe("identique au serveur");
    const toast = await waitToast("Enregistré");
    expect(await textOf(toast.$(".detail"))).toMatch(
      /^notes\.txt · 26 o · \d\d:\d\d:\d\d$/,
    );
    expect(contentOf(`${EDITOR_DIR}/notes.txt`)).toBe(
      "un\nDEUX\ntrois\nquatre\ncinq\n",
    );
    // Les marques sont recalculées 150 ms après le changement.
    await browser.waitUntil(async () => (await changedNumbers()).length === 0, {
      timeoutMsg: "marques des lignes modifiées jamais effacées",
    });
  });

  it("une nouvelle frappe efface « identique au serveur »", async () => {
    await $(TEXT).click();
    await browser.keys("y");
    await waitBar(/MODIFIÉ/i);
    expect(await textOf(BAR_DETAIL)).not.toContain("identique au serveur");
  });

  it("le bouton Enregistrer enregistre aussi", async () => {
    await $(SAVE).click();
    await waitBar(/ENREGISTRÉ À/i);
    expect(await barText()).not.toMatch(/MODIFIÉ/i);
  });
});
