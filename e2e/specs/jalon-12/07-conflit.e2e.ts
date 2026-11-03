/**
 * docs/tests-jalon-12.md, section 7 : le fichier est modifié dans la cible
 * pendant l'édition ; rien n'est écrit, puis Annuler, Recharger, Écraser et
 * recréation d'un fichier supprimé.
 */
import { $, browser, expect } from "@wdio/globals";
import {
  EDITOR_DIR,
  choose,
  contentOf,
  dialogAside,
  dialogFindings,
  dialogTitle,
  editorValue,
  openEditor,
  pressSave,
  replaceLine,
  seedEditor,
  waitBar,
} from "../../support/editor.ts";
import { sh } from "../../support/target.ts";
import {
  DIALOG,
  connect,
  dismissToasts,
  textOf,
  waitDialog,
} from "../../support/ui.ts";

const FILE = `${EDITOR_DIR}/conflit.env`;

/** Ouvre conflit.env, change B, puis le serveur change D. */
async function prepareConflict(): Promise<void> {
  seedEditor();
  await openEditor(FILE);
  await replaceLine(1, "B=vous");
  sh(`sed -i 's/D=4/D=serveur/' ${FILE}`);
}

describe("Jalon 12 · 7. Conflit", () => {
  before(async () => {
    seedEditor();
    await connect();
  });

  afterEach(async () => {
    await dismissToasts();
  });

  it("rien n'est écrit, la fenêtre dit ce que chaque choix efface", async () => {
    await prepareConflict();
    await pressSave();
    await waitDialog();
    expect(await textOf(`${DIALOG} .kicker`)).toMatch(
      /CONFLIT · CONFIRMATION REQUISE/i,
    );
    expect(await dialogAside()).toMatch(/RIEN N'A ÉTÉ ÉCRIT/i);
    expect(await dialogTitle()).toBe(
      "Le fichier a changé sur le serveur depuis son ouverture",
    );
    expect(await textOf(`${DIALOG} [data-testid="confirm-command"]`)).toMatch(
      /^Ouvert à \d\d:\d\d:\d\d · modifié sur le serveur à \d\d:\d\d:\d\d · 20 → 26 octets$/,
    );
    expect(await dialogFindings()).toEqual([
      "warn Écraser efface 1 ligne changée sur le serveur | D",
      "warn Recharger abandonne votre ligne modifiée | B",
      "neutral Pour garder les deux, ouvrez la différence et reprenez les lignes du serveur",
    ]);
    expect(contentOf(FILE)).toBe("A=1\nB=2\nC=3\nD=serveur\nE=5\n");
  });

  it("Annuler : retour à l'édition, texte intact", async () => {
    await pressSave();
    await waitDialog();
    await choose("confirm-nogo");
    expect(await editorValue()).toBe("A=1\nB=vous\nC=3\nD=4\nE=5\n");
    expect(contentOf(FILE)).toBe("A=1\nB=2\nC=3\nD=serveur\nE=5\n");
  });

  it("Recharger : le texte devient celui du serveur", async () => {
    await pressSave();
    await waitDialog();
    await choose("confirm-alternative");
    await waitBar(/^ÉDITION/i);
    expect(await editorValue()).toBe("A=1\nB=2\nC=3\nD=serveur\nE=5\n");
  });

  it("Écraser : votre version remplace celle vue dans la fenêtre", async () => {
    await prepareConflict();
    await pressSave();
    await waitDialog();
    await choose("confirm-go");
    await waitBar(/ENREGISTRÉ À/i);
    expect(contentOf(FILE)).toBe("A=1\nB=vous\nC=3\nD=4\nE=5\n");
  });

  it("Écraser après un nouveau changement du serveur : nouveau conflit", async () => {
    await prepareConflict();
    await pressSave();
    await waitDialog();
    sh(`sed -i 's/E=5/E=encore/' ${FILE}`);
    // GO : la fenêtre se referme et une nouvelle s'ouvre aussitôt.
    await $(`${DIALOG} [data-testid="confirm-go"]`).click();
    await browser.waitUntil(
      async () =>
        (await textOf(`${DIALOG} [data-testid="confirm-command"]`)).endsWith(
          "20 → 31 octets",
        ),
      { timeoutMsg: "nouveau conflit jamais affiché" },
    );
    await choose("confirm-nogo");
    expect(contentOf(FILE)).toContain("E=encore");
  });

  it("fichier supprimé sur le serveur : Recréer, avec les droits", async () => {
    seedEditor();
    sh(`chmod 600 ${FILE}`);
    await openEditor(FILE);
    await replaceLine(0, "A=recree");
    sh(`rm ${FILE}`);
    await pressSave();
    await waitDialog();
    expect(await dialogTitle()).toBe(
      "Le fichier a été supprimé sur le serveur",
    );
    await choose("confirm-go");
    await waitBar(/ENREGISTRÉ À/i);
    expect(contentOf(FILE)).toBe("A=recree\nB=2\nC=3\nD=4\nE=5\n");
    expect(sh(`stat -c %a ${FILE}`).trim()).toBe("600");
    expect(await $(DIALOG).isExisting()).toBe(false);
  });
});
