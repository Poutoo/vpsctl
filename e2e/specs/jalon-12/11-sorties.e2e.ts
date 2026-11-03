/**
 * docs/tests-jalon-12.md, section 11 : Fermer, étape de l'itinéraire, autre
 * section, Couper la liaison et fermeture de la fenêtre avec des
 * modifications ; aucune fenêtre sans modification.
 */
import { $, browser, expect } from "@wdio/globals";
import {
  CLOSE,
  EDITOR_DIR,
  TEXT,
  choose,
  dialogFindings,
  dialogTitle,
  editorValue,
  fileRow,
  openEditor,
  replaceLine,
  seedEditor,
} from "../../support/editor.ts";
import {
  DIALOG,
  button,
  connect,
  textOf,
  waitDialog,
} from "../../support/ui.ts";

const FILE = `${EDITOR_DIR}/crlf.env`;

async function modified(): Promise<void> {
  await openEditor(FILE);
  await replaceLine(2, "LOG_LEVEL=debug");
}

describe("Jalon 12 · 11. Sorties avec des modifications", () => {
  before(async () => {
    seedEditor();
    await connect();
  });

  it("Fermer : « Quitter sans enregistrer ? », NO GO garde le texte", async () => {
    await modified();
    await $(CLOSE).click();
    await waitDialog();
    expect(await dialogTitle()).toBe("Quitter sans enregistrer ?");
    expect(await textOf(`${DIALOG} [data-testid="confirm-subject"]`)).toBe(
      FILE,
    );
    expect(await dialogFindings()).toEqual([
      "warn +1 −1 lignes modifiées seront perdues | LOG_LEVEL",
      expect.stringMatching(
        /^ok Le fichier sur le serveur reste tel quel \| version du \d\d:\d\d:\d\d$/,
      ),
    ]);
    expect(await textOf(`${DIALOG} [data-testid="confirm-nogo"]`)).toMatch(
      /NO GO Rester/,
    );
    await choose("confirm-nogo");
    expect(await editorValue()).toContain("LOG_LEVEL=debug");
  });

  it("Échap vaut NO GO ; GO revient à la liste", async () => {
    await $(CLOSE).click();
    await waitDialog();
    await browser.keys("Escape");
    await $(DIALOG).waitForDisplayed({ reverse: true });
    expect(await $(TEXT).isDisplayed()).toBe(true);
    await $(CLOSE).click();
    await waitDialog();
    await choose("confirm-go");
    await fileRow("crlf.env").waitForDisplayed();
  });

  it("étape de l'itinéraire et autre section", async () => {
    await modified();
    await $(
      '//nav[@aria-label="Itinéraire"]//button[normalize-space()="editeur"]',
    ).click();
    await waitDialog();
    expect(await dialogTitle()).toBe("Quitter sans enregistrer ?");
    await choose("confirm-nogo");
    await $(
      '//nav[@aria-label="Navigation principale"]//a[contains(., "Scripts")]',
    ).click();
    await waitDialog();
    await choose("confirm-nogo");
    expect(await $(TEXT).isDisplayed()).toBe(true);
  });

  it("Couper la liaison : « Couper la liaison sans enregistrer ? »", async () => {
    await button("Couper la liaison").click();
    await waitDialog();
    expect(await dialogTitle()).toBe("Couper la liaison sans enregistrer ?");
    expect((await dialogFindings())[1]).toBe(
      "warn La liaison avec atlas-e2e sera coupée",
    );
    await choose("confirm-nogo");
    expect(await $(TEXT).isDisplayed()).toBe(true);
  });

  it("fermer la fenêtre : « Fermer Atlas sans enregistrer ? », NO GO la garde", async () => {
    await $('button[aria-label="Fermer"]').click();
    await waitDialog();
    expect(await dialogTitle()).toBe("Fermer Atlas sans enregistrer ?");
    expect((await dialogFindings())[1]).toBe(
      "warn Atlas se ferme et la liaison avec atlas-e2e est coupée",
    );
    await choose("confirm-nogo");
    expect(await $(TEXT).isDisplayed()).toBe(true);
  });

  it("sans modification : aucune fenêtre", async () => {
    await openEditor(`${EDITOR_DIR}/notes.txt`);
    await $(CLOSE).click();
    await fileRow("notes.txt").waitForDisplayed();
    expect(await $(DIALOG).isExisting()).toBe(false);
  });

  it("Couper la liaison avec des modifications : GO coupe sans redemander", async () => {
    await modified();
    await button("Couper la liaison").click();
    await waitDialog();
    await choose("confirm-go");
    await $("h1=Liaison au serveur").waitForDisplayed({ timeout: 30_000 });
    expect(await $(DIALOG).isExisting()).toBe(false);
  });
});
