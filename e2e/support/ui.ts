/**
 * Accès à l'interface d'Atlas pour les specs. Les éléments sont relus à
 * chaque appel : le tableau de bord se rafraîchit toutes les 5 s et
 * remplace ses lignes.
 */
import { $, $$, browser } from "@wdio/globals";
import { PROFILE } from "./appConfig.ts";
import { TARGET } from "./env.ts";

type ChainableElement = ReturnType<typeof $>;

/** Éléments correspondants, sous forme de tableau. */
async function all(selector: string) {
  return Array.from(await $$(selector));
}

/** Texte affiché, espaces normalisés (majuscules CSS comprises). */
export async function textOf(
  selector: string | ChainableElement,
): Promise<string> {
  const element = typeof selector === "string" ? $(selector) : selector;
  return (await element.getText()).replace(/\s+/g, " ").trim();
}

export function button(label: string): ChainableElement {
  return $(`//button[normalize-space()="${label}"]`);
}

// --- Liaison -----------------------------------------------------------------

/**
 * Établit la liaison avec un profil de test. Vérifie avant de cliquer que
 * le profil vise bien la cible locale.
 */
export async function connect(
  profileName: string = PROFILE.name,
): Promise<void> {
  const row = $(
    `//label[contains(@class,"profile")][.//span[contains(@class,"name") and normalize-space()="${profileName}"]]`,
  );
  await row.waitForDisplayed({ timeout: 30_000 });
  const endpoint = await textOf(row.$(".endpoint"));
  const expected = `${TARGET.user}@${TARGET.host}:${TARGET.port}`;
  if (endpoint !== expected) {
    throw new Error(
      `profil ${profileName} vers ${endpoint}, attendu ${expected} : liaison refusée`,
    );
  }
  await row.click();
  await button("Établir la liaison").click();
  await $('nav[aria-label="Navigation principale"]').waitForDisplayed({
    timeout: 60_000,
  });
  await waitDockerLoaded();
}

export async function disconnect(): Promise<void> {
  await button("Couper la liaison").click();
  await $("h1=Liaison au serveur").waitForDisplayed({ timeout: 30_000 });
}

/** Ferme puis relance l'application (nouvelle session WebDriver). */
export async function restartApp(): Promise<void> {
  await browser.reloadSession();
  await $("h1=Liaison au serveur").waitForDisplayed({ timeout: 60_000 });
}

export async function statusSummary(): Promise<string> {
  return textOf('[data-testid="status-summary"]');
}

// --- Navigation --------------------------------------------------------------

export async function goDashboard(): Promise<void> {
  await $(
    '//nav[@aria-label="Navigation principale"]//a[contains(., "Tableau de bord")]',
  ).click();
  await waitDockerLoaded();
}

export async function goScripts(): Promise<void> {
  await $(
    '//nav[@aria-label="Navigation principale"]//a[contains(., "Scripts")]',
  ).click();
  await $("h1=Scripts").waitForDisplayed();
}

/**
 * Écran Télémétrie, sans entrée dans la navigation : passage par
 * l'historique (Vue Router suit `popstate`), sans recharger la page.
 */
export async function goTelemetry(): Promise<void> {
  await browser.execute(() => {
    window.history.pushState({}, "", "/telemetry");
    window.dispatchEvent(new PopStateEvent("popstate"));
  });
  await $("h1=Console").waitForDisplayed();
}

// --- Tableau de bord ---------------------------------------------------------

export const DOCKER = 'section[aria-label="Conteneurs Docker"]';

export async function waitDockerLoaded(): Promise<void> {
  await browser.waitUntil(
    async () => (await $(DOCKER).getAttribute("aria-busy")) === "false",
    { timeout: 60_000, timeoutMsg: "section Docker jamais chargée" },
  );
}

/** Actualiser, puis attendre la fin de l'interrogation. */
export async function refresh(): Promise<void> {
  // Les notifications, en haut à droite, recouvrent le bouton.
  await dismissToasts();
  await button("Actualiser").click();
  await browser.pause(300);
  await browser.waitUntil(
    async () => !(await $(".refreshed.probing").isExisting()),
    {
      timeout: 30_000,
    },
  );
}

/** Remplace le filtre au clavier (`clearValue` ne déclenche pas `input` pour Vue). */
export async function setFilter(value: string): Promise<void> {
  const field = $('input[aria-label="Filtrer les tableaux"]');
  await field.click();
  await browser.keys(["Control", "a"]);
  await browser.keys("Backspace");
  if (value) await field.addValue(value);
  await browser.waitUntil(async () => (await field.getValue()) === value);
  await browser.pause(300);
}

export function project(name: string): ChainableElement {
  return $(`[data-testid="project-${name}"]`);
}

export function container(name: string): ChainableElement {
  return $(`[data-testid="container-${name}"]`);
}

export async function projectState(name: string): Promise<string> {
  return textOf(project(name).$('[data-testid="project-state"]'));
}

export async function projectDetail(name: string): Promise<string> {
  return textOf(project(name).$('[data-testid="project-detail"]'));
}

export async function projectShape(name: string): Promise<string | null> {
  const svg = project(name).$(
    '[data-testid="project-state"] [data-testid^="shape-"]',
  );
  return (await svg.getAttribute("data-testid"))?.replace("shape-", "") ?? null;
}

export async function containerState(name: string): Promise<string> {
  return textOf(container(name).$('[data-testid="container-state"]'));
}

/** Attend qu'un état de projet corresponde, en actualisant au besoin. */
export async function waitProjectState(
  name: string,
  pattern: RegExp,
  timeout = 60_000,
): Promise<void> {
  let last = "";
  await browser.waitUntil(
    async () => {
      if (!(await project(name).isExisting())) {
        last = "(projet absent)";
        return false;
      }
      last = await projectState(name);
      return pattern.test(last);
    },
    {
      timeout,
      interval: 1_000,
      timeoutMsg: `projet ${name} : état « ${last} », attendu ${pattern}`,
    },
  );
}

/** Bouton d'action de projet (« Démarrer », « Récupérer les images »…). */
export function projectAction(name: string, label: string): ChainableElement {
  return $(`button[aria-label^="${label} le projet ${name}"]`);
}

/** Raison d'indisponibilité d'un bouton d'action, ou `null` s'il est utilisable. */
export async function unavailableReason(
  target: ChainableElement,
): Promise<string | null> {
  if ((await target.getAttribute("aria-disabled")) !== "true") return null;
  const label = (await target.getAttribute("aria-label")) ?? "";
  const at = label.indexOf(" : ");
  return at === -1 ? "" : label.slice(at + 3);
}

/** Lance une action de projet et attend l'onglet Télémétrie de la commande. */
export async function runProjectAction(
  name: string,
  label: string,
): Promise<void> {
  const target = projectAction(name, label);
  await target.waitForExist();
  const reason = await unavailableReason(target);
  if (reason !== null)
    throw new Error(`${label} sur ${name} indisponible : ${reason}`);
  await target.click();
}

// --- Infobulles --------------------------------------------------------------

/** Infobulle Reka UI : survol, puis lecture de son texte. */
export async function tooltip(target: ChainableElement): Promise<string> {
  await target.moveTo();
  const tip = $(".app-tooltip");
  await tip.waitForDisplayed({ timeout: 5_000 });
  const text = await tip.getText();
  await $("h1").moveTo();
  return text.replace(/[ \t]+/g, " ").trim();
}

// --- GO / NO GO ----------------------------------------------------------------

export const DIALOG = '[role="alertdialog"]';

export async function waitDialog(): Promise<void> {
  await $(DIALOG).waitForDisplayed({ timeout: 30_000 });
}

export async function dialogText(): Promise<string> {
  return textOf(DIALOG);
}

/**
 * Lignes d'une liste du GO / NO GO (`confirm-removal`, `confirm-check`,
 * `confirm-consequence`) : « libellé | détail », sans les formes décoratives.
 */
export async function dialogItems(testid: string): Promise<string[]> {
  return browser.execute((id: string) => {
    const clean = (text: string | null | undefined) =>
      (text ?? "").replace(/\s+/g, " ").trim();
    const items = document.querySelectorAll(
      `[role="alertdialog"] [data-testid="${id}"]`,
    );
    return Array.from(items).map((item) => {
      const copy = item.cloneNode(true) as HTMLElement;
      copy.querySelectorAll('[aria-hidden="true"]').forEach((n) => n.remove());
      const label = copy.querySelector("strong");
      if (!label) return clean(copy.textContent);
      const detail = clean(copy.querySelector(".check-detail")?.textContent);
      return detail
        ? `${clean(label.textContent)} | ${detail}`
        : clean(label.textContent);
    });
  }, testid);
}

export async function go(): Promise<void> {
  await $(DIALOG).$('.//button[.//span[normalize-space()="GO"]]').click();
  await $(DIALOG).waitForDisplayed({ reverse: true });
}

export async function noGo(): Promise<void> {
  await $(DIALOG).$('.//button[.//span[normalize-space()="NO GO"]]').click();
  await $(DIALOG).waitForDisplayed({ reverse: true });
}

// --- Notifications -------------------------------------------------------------

export async function waitToast(
  title: string,
  timeout = 60_000,
): Promise<ChainableElement> {
  const toast = $(
    `//li[@data-testid="toast"][.//p[contains(@class,"title") and normalize-space()="${title}"]]`,
  );
  await toast.waitForDisplayed({ timeout });
  return toast;
}

/** Ferme les notifications ouvertes et attend la fin de leur sortie. */
export async function dismissToasts(): Promise<void> {
  const selector = 'button[aria-label="Fermer la notification"]';
  for (const close of await all(selector)) {
    // Une notification de succès peut se fermer seule entre-temps.
    await close.click().catch(() => undefined);
  }
  await browser
    .waitUntil(async () => (await all('[data-testid="toast"]')).length === 0, {
      timeout: 3_000,
    })
    .catch(() => undefined);
}

// --- Télémétrie ----------------------------------------------------------------

export async function waitTelemetryTab(name: string): Promise<void> {
  await browser.waitUntil(async () => (await activeTabName()) === name, {
    timeout: 30_000,
    timeoutMsg: `onglet « ${name} » jamais affiché`,
  });
}

export async function activeTabName(): Promise<string | null> {
  const tab = $(
    '[data-testid="telemetry-tab"].active [data-testid="tab-name"]',
  );
  return (await tab.isExisting()) ? textOf(tab) : null;
}

export async function tabNames(): Promise<string[]> {
  const tabs = await all(
    '[data-testid="telemetry-tab"] [data-testid="tab-name"]',
  );
  return Promise.all(
    tabs.map((tab) => textOf(tab as unknown as ChainableElement)),
  );
}

export async function consoleCommand(): Promise<string> {
  return (
    (await $('[data-testid="console-command"]').getAttribute("title")) ?? ""
  );
}

export async function consoleWhere(): Promise<string> {
  return textOf('[data-testid="console-where"]');
}

/** Bannière de fin de la console affichée (« LOS · Fin du flux · exit 0 »). */
export async function waitConsoleEnd(timeout = 120_000): Promise<string> {
  const banner = $('[data-testid="console-los"]');
  await banner.waitForDisplayed({ timeout });
  return textOf(banner);
}

/** Lignes visibles de la console (liste virtualisée : les dernières). */
export async function consoleLines(): Promise<
  { origin: string; text: string }[]
> {
  const lines = await all('[data-testid="console-line"]');
  return Promise.all(
    lines.map(async (line) => ({
      origin: await textOf(line.$('[data-testid="console-origin"]')),
      text: await line.$('[data-testid="console-text"]').getText(),
    })),
  );
}

// --- Couleurs et appels au backend -------------------------------------------

/** Couleur calculée d'un token (`--color-danger-text`), au format de `getCSSProperty`. */
export async function tokenColor(token: string): Promise<string> {
  return browser.execute((name: string) => {
    const probe = document.createElement("span");
    probe.style.color = `var(${name})`;
    document.body.append(probe);
    const color = getComputedStyle(probe).color;
    probe.remove();
    return color;
  }, token);
}

/** Propriété CSS calculée (couleur par défaut), au format du navigateur. */
export async function colorOf(
  target: ChainableElement | WebdriverIO.Element,
  property = "color",
): Promise<string> {
  const element = await target;
  return browser.execute(
    (node: HTMLElement, prop: string) =>
      getComputedStyle(node).getPropertyValue(prop),
    element as unknown as HTMLElement,
    property,
  );
}

/**
 * Enregistre les commandes envoyées au backend. Sous Windows, l'IPC de
 * Tauri passe par `fetch("http://ipc.localhost/<commande>")` : seul `fetch`
 * est observé, l'application n'est pas modifiée.
 */
export async function recordInvokes(): Promise<void> {
  await browser.execute(() => {
    const w = window as unknown as {
      __e2eCalls?: string[];
      __e2eFetch?: boolean;
    };
    w.__e2eCalls = [];
    if (w.__e2eFetch) return;
    w.__e2eFetch = true;
    const original = window.fetch.bind(window);
    window.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
      const url =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.href
            : input.url;
      if (url.startsWith("http://ipc.localhost/")) {
        w.__e2eCalls?.push(
          decodeURIComponent(url.slice("http://ipc.localhost/".length)),
        );
      }
      return original(input, init);
    };
  });
}

export async function invokeCalls(): Promise<string[]> {
  return browser.execute(
    () => (window as unknown as { __e2eCalls?: string[] }).__e2eCalls ?? [],
  );
}

/**
 * Vérifications d'un long scénario : toutes sont faites, puis le test
 * échoue en listant chaque écart (une première erreur ne masque pas les
 * suivantes).
 */
export function softChecks() {
  const problems: string[] = [];
  return {
    check(label: string, ok: boolean, actual?: unknown) {
      if (!ok)
        problems.push(
          actual === undefined
            ? label
            : `${label} (obtenu : ${JSON.stringify(actual)})`,
        );
    },
    assertAll() {
      if (problems.length > 0) {
        throw new Error(
          `écarts avec le scénario :\n- ${problems.join("\n- ")}`,
        );
      }
    },
  };
}
