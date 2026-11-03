/**
 * docs/tests-jalon-12.md, section 2 : ouverture par le menu, le double-clic
 * et Entrée ; itinéraire, bandeau et barre d'état ; Fermer.
 */
import { $, browser, expect } from "@wdio/globals";
import {
  BAR_DETAIL,
  CLOSE,
  EDITOR_DIR,
  STATUS,
  TEXT,
  barText,
  editorValue,
  fileRow,
  openFolder,
  seedEditor,
} from "../../support/editor.ts";
import { connect, textOf } from "../../support/ui.ts";

describe("Jalon 12 · 2. Ouverture", () => {
  before(async () => {
    seedEditor();
    await connect();
  });

  beforeEach(async () => {
    await openFolder(EDITOR_DIR);
  });

  it("« Modifier » dans le menu d'un fichier, pas d'un dossier", async () => {
    await fileRow("verrouille").click({ button: "right" });
    await $('[role="menu"]').waitForDisplayed();
    expect(
      await $(
        '//*[@role="menuitem"][.//span[normalize-space()="Modifier"]]',
      ).isExisting(),
    ).toBe(false);
    await browser.keys("Escape");

    await fileRow("crlf.env").click({ button: "right" });
    await $(
      '//*[@role="menuitem"][.//span[normalize-space()="Modifier"]]',
    ).click();
    await $(TEXT).waitForDisplayed();
    expect(await editorValue()).toBe(
      "NODE_ENV=production\nPORT=8080\nLOG_LEVEL=info\n",
    );
    expect(await textOf('[data-testid="page-surtitle"]')).toMatch(
      /^II · ITINÉRAIRE · ÉDITION$/i,
    );
    expect(await textOf("h1")).toBe("crlf.env");
    expect(await textOf('[data-testid="itinerary-position"]')).toMatch(
      /^crlf\.env ÉDITION$/i,
    );
  });

  it("bandeau « aucune modification » et barre d'état", async () => {
    await fileRow("crlf.env").doubleClick();
    await $(TEXT).waitForDisplayed();
    expect(await barText()).toMatch(/^ÉDITION/i);
    expect(await textOf(BAR_DETAIL)).toMatch(
      /^aucune modification · ouvert à \d\d:\d\d:\d\d$/,
    );
    const status = await textOf(STATUS);
    expect(status).toContain("Ligne 1, colonne 1");
    expect(status).toContain("3 lignes");
    expect(status).toMatch(/UTF-8 CRLF 48 o -rw-r--r-- 644$/);
  });

  it("double-clic et Entrée ouvrent l'éditeur ; Fermer revient à la liste", async () => {
    await fileRow("notes.txt").doubleClick();
    await $(TEXT).waitForDisplayed();
    await $(CLOSE).click();
    await fileRow("notes.txt").waitForDisplayed();
    expect(await fileRow("notes.txt").getAttribute("aria-selected")).toBe(
      "true",
    );

    await fileRow("notes.txt").click();
    await browser.keys("Enter");
    await $(TEXT).waitForDisplayed();
    expect(await textOf("h1")).toBe("notes.txt");
  });

  it("une étape de l'itinéraire ramène au dossier", async () => {
    await fileRow("notes.txt").doubleClick();
    await $(TEXT).waitForDisplayed();
    await $(
      '//nav[@aria-label="Itinéraire"]//button[normalize-space()="editeur"]',
    ).click();
    await fileRow("notes.txt").waitForDisplayed();
  });
});
