/**
 * docs/tests-jalon-12.md, section 8 : vue Différence à trois versions,
 * attribution SERVEUR / VOUS, une action par bloc, bloc mixte, Ctrl+Z,
 * « Enregistrer… » et résumés.
 */
import { $, $$, browser, expect } from "@wdio/globals";
import {
  EDITOR_DIR,
  TEXT,
  choose,
  contentOf,
  dialogFindings,
  editorValue,
  openEditor,
  pressSave,
  replaceLine,
  seedEditor,
  typeAtEnd,
  waitBar,
} from "../../support/editor.ts";
import { sh } from "../../support/target.ts";
import {
  connect,
  dismissToasts,
  textOf,
  waitDialog,
} from "../../support/ui.ts";

const FILE = `${EDITOR_DIR}/diff.env`;
const DIFF = '[data-testid="editor-diff"]';

/** Lignes des blocs : « signe texte ÉTIQUETTE ». */
async function blockRows(kind: string): Promise<string[]> {
  return browser.execute((k: string) => {
    const clean = (t: string | null) => (t ?? "").replace(/\s+/g, " ").trim();
    return Array.from(
      document.querySelectorAll(`[data-testid="diff-block-${k}"] [role="row"]`),
    ).map((row) =>
      clean(
        [".sign", ".text", ".who"]
          .map((c) => row.querySelector(c)?.textContent ?? "")
          .join(" "),
      ),
    );
  }, kind);
}

function action(kind: string) {
  return $(`[data-testid="diff-action-${kind}"]`);
}

describe("Jalon 12 · 8. Différence", () => {
  before(async () => {
    seedEditor();
    await connect();
    await openEditor(FILE);
    await replaceLine(3, "D=vous");
    await typeAtEnd("Y=1\n");
    sh(
      `sed -i -e 's/A=1/A=serveur/' -e 's/D=4/D=serveur/' -e '/F=6/d' -e '/B=2/a Z=0' ${FILE}`,
    );
  });

  afterEach(async () => {
    await dismissToasts();
  });

  it("résumés par noms de clés, puis la différence", async () => {
    await pressSave();
    await waitDialog();
    expect(await dialogFindings()).toEqual([
      "warn Écraser efface 4 lignes changées sur le serveur | A · Z · D · F",
      "warn Recharger abandonne vos 2 lignes modifiées | D · Y",
      "neutral Pour garder les deux, ouvrez la différence et reprenez les lignes du serveur",
    ]);
    await choose("confirm-link");
    await $(DIFF).waitForDisplayed();
    expect(await textOf(`${DIFF} .title`)).toMatch(
      /DIFFÉRENCE · SERVEUR → VOUS · LECTURE SEULE/i,
    );
  });

  it("attribution et une action par bloc", async () => {
    expect(await $$('[data-testid^="diff-block-"]').length).toBe(5);
    expect(await blockRows("yours")).toEqual(["+ Y=1 Vous"]);
    expect(await blockRows("mixed")).toEqual([
      "− D=serveur Serveur",
      "+ D=vous Vous",
    ]);
    expect(await textOf(action("take"))).toBe("Prendre la version du serveur");
    expect(await textOf(action("restore"))).toBe("Reprendre");
    expect(await textOf(action("remove"))).toBe("Retirer");
    expect(await textOf(action("takeMixed"))).toBe(
      "Prendre la version du serveur · efface votre ligne 4",
    );
  });

  it("teintes : ambre pour le serveur, vert-de-gris pour vous", async () => {
    const colors = await browser.execute(() => {
      const bg = (s: string) =>
        getComputedStyle(document.querySelector(s) as Element).backgroundColor;
      return {
        removed: bg('[data-testid="diff-block-mixed"] .row.removed'),
        added: bg('[data-testid="diff-block-mixed"] .row.added'),
      };
    });
    expect(colors.removed).toBe("rgba(200, 137, 59, 0.14)");
    expect(colors.added).toBe("rgba(111, 154, 139, 0.14)");
  });

  it("bloc mixte : l'action efface votre ligne, Ctrl+Z la rétablit", async () => {
    await action("takeMixed").click();
    await browser.waitUntil(async () =>
      (await editorValue()).includes("D=serveur"),
    );
    expect(await editorValue()).not.toContain("D=vous");
    await $(DIFF)
      .$(`.//button[contains(normalize-space(),"Retour à l'édition")]`)
      .click();
    await $(TEXT).click();
    await browser.keys(["Control", "z"]);
    await browser.waitUntil(async () =>
      (await editorValue()).includes("D=vous"),
    );
  });

  it("reprendre tout le serveur, puis « Enregistrer… »", async () => {
    await pressSave();
    await waitDialog();
    await choose("confirm-link");
    await $(DIFF).waitForDisplayed();
    for (const kind of ["take", "restore", "remove", "takeMixed"]) {
      await action(kind).click();
      await action(kind).waitForExist({ reverse: true });
    }
    expect(await textOf(`${DIFF} .empty`)).toBe(
      "Tous les changements du serveur sont repris : écraser ne perd plus rien.",
    );
    await $(DIFF).$('.//button[normalize-space()="Enregistrer…"]').click();
    await waitDialog();
    expect(await dialogFindings()).toEqual([
      "ok Tous les changements du serveur sont repris",
    ]);
    await choose("confirm-go");
    await waitBar(/ENREGISTRÉ À/i);
    expect(contentOf(FILE)).toBe(
      "A=serveur\nB=2\nZ=0\nC=3\nD=serveur\nE=5\nG=7\nY=1\n",
    );
  });

  it("résumés par numéros de ligne hors .env", async () => {
    seedEditor();
    const notes = `${EDITOR_DIR}/notes.txt`;
    await openEditor(notes);
    await replaceLine(0, "UN");
    sh(
      `sed -i -e 's/deux/DEUX/' -e 's/quatre/QUATRE/' -e 's/cinq/CINQ/' ${notes}`,
    );
    await pressSave();
    await waitDialog();
    expect(await dialogFindings()).toEqual([
      "warn Écraser efface 3 lignes changées sur le serveur | lignes 2, 4–5 du serveur",
      "warn Recharger abandonne votre ligne modifiée | ligne 1",
      "neutral Pour garder les deux, ouvrez la différence et reprenez les lignes du serveur",
    ]);
    await choose("confirm-nogo");
  });
});
