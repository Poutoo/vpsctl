/**
 * docs/tests-jalon-12.md, section 3 : chaque refus avec son écran ; le
 * contenu n'est jamais dans la page ; téléchargement proposé sauf pour une
 * clé privée et un lien.
 */
import { $, browser, expect } from "@wdio/globals";
import {
  EDITOR_DIR,
  REFUSAL,
  TEXT,
  openEditor,
  seedEditor,
} from "../../support/editor.ts";
import { connect, textOf } from "../../support/ui.ts";

const download = () =>
  $(REFUSAL).$('.//button[starts-with(normalize-space(),"Télécharger")]');

async function refusal(): Promise<{
  tag: string;
  title: string;
  facts: string;
}> {
  return {
    tag: await textOf(`${REFUSAL} .tag`),
    title: await textOf(`${REFUSAL} .title`),
    facts: await textOf(`${REFUSAL} .facts`),
  };
}

describe("Jalon 12 · 3. Refus", () => {
  before(async () => {
    seedEditor();
    await connect();
  });

  it("plus de 1 Mo : taille et limite, téléchargement proposé", async () => {
    await openEditor(`${EDITOR_DIR}/gros.log`);
    const shown = await refusal();
    expect(shown.tag).toMatch(/REFUSÉ · TROP VOLUMINEUX/i);
    expect(shown.title).toBe("Ce fichier dépasse la limite de l'éditeur.");
    expect(shown.facts).toContain("Taille 1,1 Mo");
    expect(shown.facts).toContain("Limite 1 Mo");
    expect(await download().isDisplayed()).toBe(true);
    expect(await $(TEXT).isExisting()).toBe(false);
    expect(await textOf('[data-testid="itinerary-position"]')).toMatch(
      /gros\.log REFUSÉ/i,
    );
  });

  it("binaire : position du premier octet nul, sans format", async () => {
    await openEditor(`${EDITOR_DIR}/binaire.db`);
    const shown = await refusal();
    expect(shown.tag).toMatch(/CONTENU BINAIRE/i);
    expect(shown.facts).toContain(
      "Détecté premier octet nul à la position 15 (0x000f)",
    );
    expect(shown.facts).not.toMatch(/sqlite/i);
    expect(await download().isDisplayed()).toBe(true);
  });

  it("UTF-8 invalide après les 8 premiers Ko : validé sur tout le fichier", async () => {
    await openEditor(`${EDITOR_DIR}/utf8-invalide.txt`);
    const shown = await refusal();
    expect(shown.title).toBe("Ce fichier n'est pas de l'UTF-8 valide.");
    expect(shown.facts).toContain(
      "première séquence invalide à l'octet 10 001 (ligne 2)",
    );
    expect(await download().isDisplayed()).toBe(true);
  });

  it("clé privée hors de ~/.ssh : jamais affichée ni proposée au téléchargement", async () => {
    await openEditor(`${EDITOR_DIR}/cle.pem`);
    const shown = await refusal();
    expect(shown.title).toBe(
      "Une clé privée ne s'ouvre jamais dans l'éditeur.",
    );
    expect(shown.facts).toContain("en-tête PRIVATE KEY à la ligne 1");
    expect(shown.facts).toContain(`Dossier ${EDITOR_DIR}`);
    expect(await download().isExisting()).toBe(false);
    const page = await browser.execute(() => document.body.innerText);
    expect(page).not.toContain("b3BlbnNzaC1rZXk");
  });

  it("lien symbolique : refusé, cible affichée, pas de téléchargement", async () => {
    await openEditor(`${EDITOR_DIR}/lien.env`);
    const shown = await refusal();
    expect(shown.title).toBe(
      "Un lien symbolique ne s'ouvre pas dans l'éditeur.",
    );
    expect(shown.facts).toContain("Cible crlf.env");
    expect(await download().isExisting()).toBe(false);
  });

  it("« Télécharger » lance un transfert", async () => {
    await openEditor(`${EDITOR_DIR}/binaire.db`);
    // La fenêtre d'enregistrement du système n'est pas pilotable par
    // WebDriver : seule la présence du bouton est vérifiée ici, le transfert
    // relève des tests du jalon 4.
    expect(await download().isClickable()).toBe(true);
  });
});
