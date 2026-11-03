/**
 * docs/tests-jalon-12.md, section 4 : chaque raison de la lecture seule,
 * affichée avec ses faits ; pas d'enregistrement, contenu lisible.
 */
import { $, browser, expect } from "@wdio/globals";
import {
  BAR_DETAIL,
  EDITOR_DIR,
  READ_ONLY,
  SAVE,
  STATUS,
  TEXT,
  barText,
  contentOf,
  openEditor,
  pressSave,
  seedEditor,
  typeAtEnd,
} from "../../support/editor.ts";
import { connect, textOf } from "../../support/ui.ts";

async function notice(): Promise<string> {
  return textOf(READ_ONLY);
}

describe("Jalon 12 · 4. Lecture seule", () => {
  before(async () => {
    seedEditor();
    await connect();
  });

  it("droits insuffisants (444)", async () => {
    await openEditor(`${EDITOR_DIR}/lecture-seule.txt`);
    expect(await barText()).toMatch(/^LECTURE SEULE/i);
    expect(await textOf(BAR_DETAIL)).toMatch(/^droits insuffisants · ouvert à/);
    const text = await notice();
    expect(text).toMatch(/LECTURE SEULE · DROITS INSUFFISANTS/i);
    expect(text).toContain(
      "L'utilisateur atlas n'a pas le droit d'écrire dans ce fichier.",
    );
    expect(text).toContain(
      "Droits -r--r--r-- 444 · propriétaire atlas:atlas · connecté en atlas · dossier",
    );
    expect(await $(SAVE).isExisting()).toBe(false);
    expect(await $(TEXT).getAttribute("readonly")).not.toBeNull();
    expect(await textOf(STATUS)).toMatch(/lecture seule$/);
  });

  it("autre propriétaire (compte autre)", async () => {
    await openEditor(`${EDITOR_DIR}/autre.conf`);
    const text = await notice();
    expect(text).toMatch(/AUTRE PROPRIÉTAIRE/i);
    expect(text).toContain(
      "Ce fichier appartient à autre ; vous êtes connecté en atlas.",
    );
    expect(text).toContain("Atlas ne change pas le propriétaire d'un fichier.");
    expect(text).toContain("propriétaire autre:autre");
  });

  it("dossier non inscriptible (555)", async () => {
    await openEditor(`${EDITOR_DIR}/verrouille/reglages.env`);
    const text = await notice();
    expect(text).toMatch(/DOSSIER NON INSCRIPTIBLE/i);
    expect(text).toContain(
      `Le dossier ${EDITOR_DIR}/verrouille appartient à atlas et n'est pas inscriptible par atlas.`,
    );
    expect(text).toContain("dr-xr-xr-x");
  });

  it("fins de ligne mélangées", async () => {
    await openEditor(`${EDITOR_DIR}/mixte.txt`);
    expect(await notice()).toMatch(/FINS DE LIGNE MÉLANGÉES/i);
    expect(await textOf(STATUS)).toContain("CRLF + LF");
  });

  it("le texte reste lisible ; Ctrl+S n'écrit rien", async () => {
    await openEditor(`${EDITOR_DIR}/lecture-seule.txt`);
    expect(await $(TEXT).getValue()).toBe("version: 1\n");
    await typeAtEnd("ajout");
    await pressSave();
    await browser.pause(500);
    expect(await $(TEXT).getValue()).toBe("version: 1\n");
    expect(contentOf(`${EDITOR_DIR}/lecture-seule.txt`)).toBe("version: 1\n");
  });
});
