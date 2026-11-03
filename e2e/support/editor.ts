/**
 * Éditeur de fichiers pour les specs du jalon 12 : fichiers de la cible,
 * ouverture, saisie, enregistrement et fenêtres GO / NO GO.
 */
import { execFileSync } from "node:child_process";
import { $, $$, browser } from "@wdio/globals";
import { TARGET } from "./env.ts";
import { sh } from "./target.ts";
import { DIALOG, textOf } from "./ui.ts";

type ChainableElement = ReturnType<typeof $>;

/** Dossier des fichiers de test de l'éditeur, dans la cible. */
export const EDITOR_DIR = `${TARGET.home}/atlas-tests/editeur`;

export const TEXT = '[data-testid="editor-text"]';
export const BAR = '[data-testid="editor-bar"]';
export const BAR_DETAIL = '[data-testid="editor-bar-detail"]';
export const REFUSAL = '[data-testid="editor-refusal"]';
export const READ_ONLY = '[data-testid="editor-read-only"]';
export const STATUS = '[data-testid="editor-status"]';
export const SAVE = '[data-testid="editor-save"]';
export const CLOSE = '[data-testid="editor-close"]';

/** Remet les fichiers de test (et authorized_keys) à l'état de départ. */
export function seedEditor(what: "all" | "keys" = "all"): void {
  execFileSync(
    "docker",
    ["exec", TARGET.container, "bash", "/e2e/seed-editor.sh", what],
    { stdio: ["ignore", "pipe", "pipe"], timeout: 60_000 },
  );
}

/** Contenu brut d'un fichier de la cible, octet par octet (`od -An -c`). */
export function bytesOf(file: string): string {
  return sh(`od -An -c ${file} | tr -s ' '`).replace(/\s+/g, " ").trim();
}

export function contentOf(file: string): string {
  return sh(`cat ${file}`);
}

/** Change d'écran sans recharger la page (Vue Router suit `popstate`). */
export async function goTo(url: string): Promise<void> {
  await browser.execute((target: string) => {
    window.history.pushState({}, "", target);
    window.dispatchEvent(new PopStateEvent("popstate"));
  }, url);
}

export async function openFolder(dir: string): Promise<void> {
  await goTo(`/files${dir}`);
  await $('[role="grid"]').waitForDisplayed();
}

/** Ligne de la liste de l'explorateur. */
export function fileRow(name: string): ChainableElement {
  return $(
    `//div[@role="row"][.//*[contains(@class,"name") and normalize-space()="${name}"]]`,
  );
}

/**
 * Une sortie de l'éditeur avec des modifications demande « Quitter sans
 * enregistrer ? » : GO, pour repartir d'un état connu.
 */
async function discardIfAsked(): Promise<void> {
  const shown = await $(DIALOG)
    .waitForDisplayed({ timeout: 1_000 })
    .then(() => true)
    .catch(() => false);
  if (!shown) return;
  await $(`${DIALOG} [data-testid="confirm-go"]`).click();
  await $(DIALOG).waitForDisplayed({ reverse: true });
}

/**
 * Ouvre l'éditeur par l'URL et attend son contenu (texte, refus ou lecture
 * seule). Passe d'abord par le dossier : un fichier déjà ouvert est relu, et
 * des modifications en cours sont abandonnées.
 */
export async function openEditor(file: string): Promise<void> {
  const dir = file.slice(0, file.lastIndexOf("/")) || "/";
  await goTo(`/files${dir}`);
  await discardIfAsked();
  await $('[role="grid"]').waitForDisplayed();
  await goTo(`/files${file}?edition`);
  await browser.waitUntil(
    async () =>
      (await $(TEXT)
        .isDisplayed()
        .catch(() => false)) ||
      (await $(REFUSAL)
        .isDisplayed()
        .catch(() => false)),
    { timeout: 30_000, timeoutMsg: `${file} : éditeur jamais affiché` },
  );
  // Texte ouvert : l'éditeur place le curseur au début, juste après
  // l'affichage. La saisie du test attend ce moment.
  if (await $(TEXT).isExisting()) {
    await browser.waitUntil(
      () =>
        browser.execute(() => {
          const area = document.querySelector<HTMLTextAreaElement>(
            '[data-testid="editor-text"]',
          );
          return document.activeElement === area && area?.selectionStart === 0;
        }),
      { timeoutMsg: `${file} : curseur jamais placé au début du texte` },
    );
  }
}

export async function editorValue(): Promise<string> {
  return $(TEXT).getValue();
}

/** Place le curseur en fin de texte et tape `text` (touches réelles). */
export async function typeAtEnd(text: string): Promise<void> {
  await $(TEXT).click();
  await browser.keys(["Control", "End"]);
  await browser.keys(text);
}

/**
 * Remplace le texte d'une ligne (index à partir de 0), au clavier :
 * sélection de la ligne par Début, Maj+Fin, puis saisie.
 */
export async function replaceLine(index: number, text: string): Promise<void> {
  await $(TEXT).click();
  await browser.keys(["Control", "Home"]);
  for (let i = 0; i < index; i += 1) await browser.keys("ArrowDown");
  await browser.keys("Home");
  await browser.keys(["Shift", "End"]);
  await browser.keys(text);
}

export async function pressSave(): Promise<void> {
  await browser.keys(["Control", "s"]);
}

export async function barText(): Promise<string> {
  return textOf(BAR);
}

/** Attend que le bandeau du cadre affiche `label` (début du texte). */
export async function waitBar(label: RegExp, timeout = 30_000): Promise<void> {
  await browser.waitUntil(async () => label.test(await barText()), {
    timeout,
    timeoutMsg: `bandeau jamais « ${label} »`,
  });
}

/** Constats du GO / NO GO (`confirm-item-*`) : « libellé | détail ». */
export async function dialogFindings(): Promise<string[]> {
  return browser.execute(() => {
    const clean = (text: string | null | undefined) =>
      (text ?? "").replace(/\s+/g, " ").trim();
    return Array.from(
      document.querySelectorAll(
        '[role="alertdialog"] [data-testid^="confirm-item-"]',
      ),
    ).map((item) => {
      const tone = item
        .getAttribute("data-testid")
        ?.replace("confirm-item-", "");
      const label = clean(item.querySelector("strong")?.textContent);
      const detail = clean(item.querySelector(".item-detail")?.textContent);
      return detail ? `${tone} ${label} | ${detail}` : `${tone} ${label}`;
    });
  });
}

export async function dialogTitle(): Promise<string> {
  return textOf(`${DIALOG} .title`);
}

export async function dialogAside(): Promise<string> {
  return textOf(`${DIALOG} [data-testid="confirm-aside"]`);
}

export function dialogButton(testid: string): ChainableElement {
  return $(`${DIALOG} [data-testid="${testid}"]`);
}

export async function choose(
  testid:
    "confirm-go" | "confirm-nogo" | "confirm-alternative" | "confirm-link",
): Promise<void> {
  await dialogButton(testid).click();
  await $(DIALOG).waitForDisplayed({ reverse: true });
}

export async function lineNumbers(): Promise<string[]> {
  const numbers = Array.from(await $$(".gutter .number"));
  return Promise.all(numbers.map((n) => n.getText()));
}

export async function changedNumbers(): Promise<string[]> {
  const numbers = Array.from(await $$(".gutter .number.changed"));
  return Promise.all(numbers.map((n) => n.getText()));
}
