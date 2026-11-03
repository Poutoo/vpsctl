/**
 * docs/tests-jalon-12.md, section 9 : bandeau des fichiers de ~/.ssh,
 * GO / NO GO d'authorized_keys dans ses trois états, lignes avec options,
 * certificat et ligne illisible, case décochée à chaque enregistrement.
 * La cible garde la clé de test : chaque test la remet en place (root).
 */
import { $, browser, expect } from "@wdio/globals";
import {
  choose,
  contentOf,
  dialogAside,
  dialogButton,
  dialogFindings,
  dialogTitle,
  openEditor,
  pressSave,
  seedEditor,
  typeAtEnd,
  waitBar,
} from "../../support/editor.ts";
import { TARGET } from "../../support/env.ts";
import { sh } from "../../support/target.ts";
import {
  DIALOG,
  connect,
  dismissToasts,
  textOf,
  waitDialog,
} from "../../support/ui.ts";

const KEYS = `${TARGET.home}/.ssh/authorized_keys`;
const BANNER = '[data-testid="editor-ssh-banner"]';
const ACK = `${DIALOG} [data-testid="confirm-acknowledge"]`;

/** Lignes du fichier qui ne sont pas la clé de test (connue par son base64). */
function withoutLinkKey(): string {
  const linkKey = sh(`awk '{print $2}' /e2e/authorized_keys`, {
    user: "root",
  }).trim();
  return contentOf(KEYS)
    .split("\n")
    .filter((line) => !line.includes(linkKey))
    .join("\n");
}

describe("Jalon 12 · 9. authorized_keys et ~/.ssh", () => {
  before(async () => {
    seedEditor();
    sh(
      `printf 'Host *\\n  ServerAliveInterval 30\\n' > ${TARGET.home}/.ssh/config`,
    );
    await connect();
  });

  afterEach(async () => {
    await dismissToasts();
    seedEditor("keys");
  });

  it("autre fichier de ~/.ssh : bandeau seul, sans GO / NO GO", async () => {
    await openEditor(`${TARGET.home}/.ssh/config`);
    expect(await textOf(BANNER)).toMatch(/^FICHIER SSH ·/i);
    await typeAtEnd("# test");
    await pressSave();
    await waitBar(/ENREGISTRÉ À/i);
    expect(await $(DIALOG).isExisting()).toBe(false);
  });

  it("bandeau permanent d'authorized_keys", async () => {
    await openEditor(KEYS);
    expect(await textOf(BANNER)).toBe(
      "FICHIER D'ACCÈS SSH · Une erreur ici peut vous empêcher de vous reconnecter à atlas-e2e, Atlas compris. Chaque enregistrement passe par un GO / NO GO.",
    );
  });

  it("tout passe : clé conservée, options, certificat, ligne non reconnue", async () => {
    await openEditor(KEYS);
    await typeAtEnd("# commentaire\n");
    await pressSave();
    await waitDialog();
    expect(await dialogTitle()).toBe("Enregistrer un fichier d'accès SSH ?");
    expect(await dialogAside()).toMatch(/ACCÈS CONSERVÉ/i);
    const findings = await dialogFindings();
    expect(findings[0]).toMatch(
      /^ok La clé de cette liaison reste autorisée · ligne 2 \| ED25519 · atlas-e2e · …\S{8}$/,
    );
    expect(findings[1]).toBe(
      "warn 4 clés reconnues | dont 1 certificat · ligne 6 non reconnue par Atlas",
    );
    expect(findings[2]).toBe(
      "ok Droits conservés | 600 · sshd refuse un fichier trop ouvert",
    );
    expect(await dialogButton("confirm-go").isEnabled()).toBe(false);
    await $(ACK).click();
    expect(await dialogButton("confirm-go").isEnabled()).toBe(true);
    await choose("confirm-go");
    await waitBar(/ENREGISTRÉ À/i);
    expect(contentOf(KEYS)).toContain("# commentaire");
    expect(sh(`stat -c %a ${KEYS}`).trim()).toBe("600");
  });

  it("la case se décoche à chaque enregistrement", async () => {
    await openEditor(KEYS);
    await typeAtEnd("# un\n");
    await pressSave();
    await waitDialog();
    await $(ACK).click();
    await choose("confirm-go");
    await waitBar(/ENREGISTRÉ À/i);
    await typeAtEnd("# deux\n");
    await pressSave();
    await waitDialog();
    expect(await $(ACK).isSelected()).toBe(false);
    expect(await dialogButton("confirm-go").isEnabled()).toBe(false);
    await choose("confirm-nogo");
  });

  it("clé de la liaison retirée : avertissement fort, NO GO n'écrit rien", async () => {
    await openEditor(KEYS);
    const before = contentOf(KEYS);
    await browser.execute((text: string) => {
      const area = document.querySelector<HTMLTextAreaElement>(
        '[data-testid="editor-text"]',
      );
      if (!area) return;
      area.focus();
      area.select();
      document.execCommand("insertText", false, text);
    }, withoutLinkKey());
    await pressSave();
    await waitDialog();
    expect(await dialogTitle()).toBe(
      "Enregistrer sans la clé de cette liaison ?",
    );
    expect(await dialogAside()).toMatch(/PERTE D'ACCÈS PROBABLE/i);
    expect(await textOf(`${DIALOG} [data-testid="confirm-alert"]`)).toMatch(
      /^LIAISON EN COURS MENACÉE ·/i,
    );
    expect((await dialogFindings())[0]).toMatch(
      /^danger La clé de cette liaison est retirée du fichier \| ED25519 · atlas-e2e · …\S{8} · ligne 2 à l'ouverture$/,
    );
    expect(await $(`${DIALOG} .acknowledge.danger`).isDisplayed()).toBe(true);
    await choose("confirm-nogo");
    expect(contentOf(KEYS)).toBe(before);
  });

  it("clé absente avant comme après : constat neutre, case requise", async () => {
    // La liaison en cours reste ouverte : seule la prochaine serait refusée.
    sh(`sed -i '2d' ${KEYS}`, { user: "root" });
    await openEditor(KEYS);
    await typeAtEnd("# ajout\n");
    await pressSave();
    await waitDialog();
    expect(await dialogAside()).toMatch(/CLÉ ABSENTE AVANT ET APRÈS/i);
    expect((await dialogFindings())[0]).toMatch(
      /^neutral Clé de cette liaison introuvable avant comme après \| Absente à l'ouverture/,
    );
    expect(await dialogButton("confirm-go").isEnabled()).toBe(false);
    await choose("confirm-nogo");
  });
});
