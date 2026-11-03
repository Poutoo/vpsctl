/**
 * docs/tests-jalon-12.md, section 10 : LOS pendant une édition (cible
 * gelée par `docker pause`, comme au jalon 11) ; texte conservé,
 * enregistrement désactivé, puis vérification de conflit au rétablissement.
 */
import { $, browser, expect } from "@wdio/globals";
import {
  BAR_DETAIL,
  EDITOR_DIR,
  SAVE,
  choose,
  contentOf,
  dialogTitle,
  editorValue,
  openEditor,
  pressSave,
  replaceLine,
  seedEditor,
  waitBar,
} from "../../support/editor.ts";
import { freezeTarget, sh, thawTarget } from "../../support/target.ts";
import {
  button,
  connect,
  dismissToasts,
  textOf,
  waitDialog,
  waitToast,
} from "../../support/ui.ts";

const FILE = `${EDITOR_DIR}/conflit.env`;

describe("Jalon 12 · 10. Perte de liaison", () => {
  before(async () => {
    seedEditor();
    await connect();
  });

  after(() => {
    // Déjà dégelée par le second test, sauf s'il a échoué avant.
    try {
      thawTarget();
    } catch {
      // cible déjà active
    }
  });

  afterEach(async () => {
    await dismissToasts();
  });

  it("LOS : texte conservé, enregistrement indisponible", async () => {
    await openEditor(FILE);
    await replaceLine(1, "B=vous");
    freezeTarget();
    await waitBar(/^▲? ?LIAISON PERDUE · \+1 −1 LIGNES/i, 60_000);
    expect(await textOf(BAR_DETAIL)).toBe(
      "texte conservé · enregistrement indisponible",
    );
    expect(await $(SAVE).isEnabled()).toBe(false);
    expect(await $(".frame.frozen").isDisplayed()).toBe(true);
    await pressSave();
    await browser.pause(500);
    expect(await editorValue()).toBe("A=1\nB=vous\nC=3\nD=4\nE=5\n");
    await replaceLine(2, "C=pendant");
    expect(await editorValue()).toContain("C=pendant");
  });

  it("rétablie : l'enregistrement repasse par la vérification de conflit", async () => {
    thawTarget();
    sh(`sed -i 's/E=5/E=serveur/' ${FILE}`);
    await button("Rétablir la liaison").click();
    await browser.waitUntil(async () => $(SAVE).isEnabled(), {
      timeout: 60_000,
      timeoutMsg: "Enregistrer jamais réactivé",
    });
    expect(await editorValue()).toBe("A=1\nB=vous\nC=pendant\nD=4\nE=5\n");
    const toast = await waitToast("Liaison rétablie");
    expect(await textOf(toast.$(".detail"))).toBe(
      "AOS · atlas-e2e · fichier à revérifier",
    );
    await dismissToasts();
    await pressSave();
    await waitDialog();
    expect(await dialogTitle()).toBe(
      "Le fichier a changé sur le serveur depuis son ouverture",
    );
    await choose("confirm-go");
    await waitBar(/ENREGISTRÉ À/i);
    expect(contentOf(FILE)).toBe("A=1\nB=vous\nC=pendant\nD=4\nE=5\n");
  });
});
