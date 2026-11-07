/**
 * Capture d'un échec, pour toutes les specs : image de la fenêtre, état de
 * l'écran (en-tête, bandeaux, fenêtres GO / NO GO ou de conflit,
 * notifications), journal de l'application et fichiers de la cible
 * concernés, lus octet par octet. Rangée dans e2e/.cache/echecs/, gardée
 * d'un lancement à l'autre.
 *
 * L'application n'écrit pas de journal : on en tient un dans la page, aux
 * appels du backend (commande, réussite ou erreur et son contenu) et aux
 * erreurs de la console. Comme `recordInvokes`, seul `fetch` est observé.
 */
import fs from "node:fs";
import path from "node:path";
import { browser } from "@wdio/globals";
import { CACHE_DIR } from "./env.ts";
import { sh } from "./target.ts";

const FAILURES_DIR = path.join(CACHE_DIR, "echecs");
const JOURNAL_LIMIT = 300;

/**
 * Fichiers de la cible lus à l'échec du test en cours. Sur `globalThis` :
 * la configuration et les specs ne passent pas par le même chargeur de
 * modules, chacun aurait sa propre copie de ce module.
 */
const shared = globalThis as { __e2eNotedFiles?: Set<string> };
const notedFiles = (shared.__e2eNotedFiles ??= new Set<string>());

export function noteFile(file: string): void {
  notedFiles.add(file);
}

export function resetNotedFiles(): void {
  notedFiles.clear();
}

/**
 * Tient le journal de la page (une fois par page : à refaire après une
 * relance de l'application).
 */
export async function installJournal(): Promise<void> {
  await browser
    .execute((limit: number) => {
      type Entry = { at: string; kind: string; detail: string };
      const w = window as unknown as {
        __e2eJournal?: Entry[];
        __e2eJournalInstalled?: boolean;
      };
      if (w.__e2eJournalInstalled) return;
      w.__e2eJournalInstalled = true;
      w.__e2eJournal = [];
      const push = (kind: string, detail: string) => {
        const journal = w.__e2eJournal!;
        journal.push({
          at: new Date().toISOString().slice(11, 23),
          kind,
          detail: detail.slice(0, 2_000),
        });
        if (journal.length > limit) journal.shift();
      };
      const original = window.fetch.bind(window);
      window.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
        const url =
          typeof input === "string"
            ? input
            : input instanceof URL
              ? input.href
              : input.url;
        if (!url.startsWith("http://ipc.localhost/"))
          return original(input, init);
        const command = decodeURIComponent(
          url.slice("http://ipc.localhost/".length),
        );
        try {
          const response = await original(input, init);
          if (response.ok) push("appel", `${command} · ok`);
          else
            push(
              "appel",
              `${command} · ${response.status} ${await response.clone().text()}`,
            );
          return response;
        } catch (error) {
          push("appel", `${command} · échec du transport : ${String(error)}`);
          throw error;
        }
      };
      for (const level of ["error", "warn"] as const) {
        const log = console[level].bind(console);
        console[level] = (...args: unknown[]) => {
          push(`console.${level}`, args.map(String).join(" "));
          log(...args);
        };
      }
      window.addEventListener("error", (event) =>
        push("erreur", `${event.message} (${event.filename}:${event.lineno})`),
      );
      window.addEventListener("unhandledrejection", (event) =>
        push("promesse rejetée", String(event.reason)),
      );
    }, JOURNAL_LIMIT)
    .catch(() => undefined);
}

/** État de l'écran tel qu'un humain le lirait. */
async function screenState(): Promise<unknown> {
  return browser.execute(() => {
    const clean = (text: string | null | undefined) =>
      (text ?? "").replace(/\s+/g, " ").trim();
    const text = (selector: string) =>
      clean(document.querySelector<HTMLElement>(selector)?.innerText) || null;
    const all = (selector: string) =>
      Array.from(document.querySelectorAll<HTMLElement>(selector)).map((e) =>
        clean(e.innerText),
      );
    return {
      url: location.pathname + location.search,
      titre: text("h1"),
      surtitre: text('[data-testid="page-surtitle"]'),
      bandeauEditeur: text('[data-testid="editor-bar"]'),
      detailBandeau: text('[data-testid="editor-bar-detail"]'),
      barreEditeur: text('[data-testid="editor-status"]'),
      barreEtat: text('[data-testid="status-summary"]'),
      fenetres: all('[role="alertdialog"], [role="dialog"]'),
      notifications: all('[data-testid="toast"]'),
      focus: document.activeElement
        ? `${document.activeElement.tagName} ${document.activeElement.getAttribute("data-testid") ?? ""}`
        : null,
    };
  });
}

function slug(text: string): string {
  return text
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^\w]+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 80);
}

/**
 * Rassemble tout ce qui aide à comprendre un échec. Ne lève jamais : une
 * partie impossible à capturer est notée à sa place.
 */
export async function captureFailure(
  spec: string,
  title: string,
  error: unknown,
): Promise<void> {
  const stamp = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
  const dir = path.join(
    FAILURES_DIR,
    `${stamp}-${slug(path.basename(spec, ".e2e.ts"))}`,
    slug(title),
  );
  fs.mkdirSync(dir, { recursive: true });
  const write = (name: string, content: string) =>
    fs.writeFileSync(path.join(dir, name), content);
  const attempt = async (name: string, task: () => Promise<string | void>) => {
    try {
      const content = await task();
      if (content !== undefined) write(name, content);
    } catch (failure) {
      write(name, `capture impossible : ${String(failure)}\n`);
    }
  };

  write(
    "erreur.txt",
    `${title}\n\n${error instanceof Error ? (error.stack ?? error.message) : String(error)}\n`,
  );
  await attempt("fenetre.png", async () => {
    await browser.saveScreenshot(path.join(dir, "fenetre.png"));
  });
  await attempt("ecran.json", async () =>
    JSON.stringify(await screenState(), null, 2),
  );
  await attempt("journal.json", async () =>
    JSON.stringify(
      await browser.execute(
        () =>
          (window as unknown as { __e2eJournal?: unknown[] }).__e2eJournal ??
          "journal absent (page rechargée sans installJournal)",
      ),
      null,
      2,
    ),
  );
  await attempt("fichiers.txt", async () => {
    if (notedFiles.size === 0) return "aucun fichier noté pour ce test\n";
    return [...notedFiles]
      .map((file) => {
        try {
          return `== ${file}\n${sh(
            `stat -c '%A %a %U:%G %s o, modifié %y' '${file}'; od -An -c -N 4096 '${file}'`,
            { user: "root" },
          )}`;
        } catch (failure) {
          return `== ${file}\nlecture impossible : ${String(failure)}`;
        }
      })
      .join("\n");
  });
  console.warn(`[e2e] échec capturé dans ${dir}`);
}
