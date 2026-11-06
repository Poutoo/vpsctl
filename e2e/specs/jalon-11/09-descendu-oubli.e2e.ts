/** docs/tests-jalon-11.md, section 9 : projet descendu, oubli. */
import { $, browser, expect } from "@wdio/globals";
import {
  OTHER_PROFILE,
  PROFILE,
  rememberedProjects,
} from "../../support/appConfig.ts";
import { TARGET } from "../../support/env.ts";
import { seed, sh, volumes } from "../../support/target.ts";
import {
  button,
  connect,
  dialogText,
  disconnect,
  go,
  goDashboard,
  project,
  projectAction,
  projectDetail,
  projectShape,
  refresh,
  restartApp,
  runProjectAction,
  textOf,
  unavailableReason,
  waitConsoleEnd,
  waitDialog,
  waitProjectState,
  waitTelemetryTab,
  waitToast,
} from "../../support/ui.ts";

/** Descend un projet hors d'Atlas, après qu'Atlas l'a vu (et mémorisé). */
async function downOutside(name: string): Promise<void> {
  await project(name).waitForExist();
  sh(`cd /tmp && docker compose -p ${name} down -t 0`);
  await refresh();
  await waitProjectState(name, /^DESCENDU/i);
}

describe("Jalon 11 · 9. Projet descendu, oubli", () => {
  before(async () => {
    seed();
    await connect();
  });

  it("après un down : Descendu · 0/N, cercle vide, explication, actions et Oublier", async () => {
    await downOutside("nominal");
    expect(
      await textOf(project("nominal").$('[data-testid="project-state"]')),
    ).toMatch(/^DESCENDU · 0\/2$/i);
    expect(await projectShape("nominal")).toBe("circle");
    expect(await projectDetail("nominal")).toBe("aucun conteneur · mémorisé");
    expect(await textOf('[data-testid="project-down-nominal"]')).toMatch(
      /^Aucun conteneur\. Docker ne connaît plus ce projet/,
    );
    expect(
      await unavailableReason(projectAction("nominal", "Démarrer")),
    ).toBeNull();
    expect(
      await unavailableReason(projectAction("nominal", "Récupérer les images")),
    ).toBeNull();
    expect(
      await unavailableReason(projectAction("nominal", "Redémarrer")),
    ).toBe("aucun conteneur");
    expect(
      await $('button[aria-label^="Oublier le projet nominal"]').isExisting(),
    ).toBe(true);
    expect(
      await projectAction("nominal", "Arrêter et supprimer").isExisting(),
    ).toBe(false);
  });

  it("Démarrer sur le projet descendu : recréé depuis son dossier, Nominal", async () => {
    await runProjectAction("nominal", "Démarrer");
    await waitTelemetryTab("up · nominal");
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);
    await goDashboard();
    await waitProjectState("nominal", /^NOMINAL · 2\/2$/i);
  });

  it("persistance : toujours là après relance d'Atlas, absent sous un autre profil", async () => {
    await downOutside("nominal");
    await restartApp();
    await connect();
    await waitProjectState("nominal", /^DESCENDU · 0\/2$/i);

    await disconnect();
    await connect(OTHER_PROFILE.name);
    await browser.pause(1_000);
    expect(await project("nominal").isExisting()).toBe(false);
    await disconnect();
    await connect();
  });

  it("Oublier : GO / NO GO simple, projet retiré, rien touché sur le serveur", async () => {
    const files = sh(`ls -A ${TARGET.tests}/nominal`);
    const before = volumes();

    await $('button[aria-label^="Oublier le projet nominal"]').click();
    await waitDialog();
    const text = await dialogText();
    expect(text).toMatch(/Oublier ce projet \?/i);
    expect(text).toContain("Rien n'est touché sur le serveur");
    expect(
      await $(
        '[role="alertdialog"] [data-testid="confirm-command"]',
      ).isExisting(),
    ).toBe(false);
    await go();

    await waitToast("Projet oublié");
    await project("nominal").waitForExist({ reverse: true });
    expect(sh(`ls -A ${TARGET.tests}/nominal`)).toBe(files);
    expect(volumes()).toEqual(before);
  });

  it("supprimer un profil (déconnecté) : ses projets mémorisés disparaissent", async () => {
    expect(Object.keys(rememberedProjects())).toContain(OTHER_PROFILE.id);
    await disconnect();
    await $('//a[normalize-space()="Nouveau profil"]').click();
    await $(
      `//button[.//span[normalize-space()="${OTHER_PROFILE.name}"]]`,
    ).click();
    await button("Supprimer…").click();
    await waitDialog();
    await go();
    await browser.waitUntil(() => !(OTHER_PROFILE.id in rememberedProjects()), {
      timeout: 10_000,
      timeoutMsg:
        "projets du profil supprimé encore dans compose-projects.json",
    });
    expect(Object.keys(rememberedProjects())).toContain(PROFILE.id);
  });
});
