/** docs/tests-jalon-11.md, section 11 : limites des onglets et des flux. */
import { $, browser, expect } from "@wdio/globals";
import { seed, sh } from "../../support/target.ts";
import {
  button,
  connect,
  dismissToasts,
  DIALOG,
  go,
  goDashboard,
  goTelemetry,
  refresh,
  runProjectAction,
  tabNames,
  textOf,
  waitConsoleEnd,
  waitProjectState,
  waitTelemetryTab,
  waitToast,
} from "../../support/ui.ts";

const COMPOSE_TAB = /^(up|pull|restart|down) · /;

/** Ouvre les logs d'un conteneur dans le panneau ancré, puis en plein écran. */
async function openLogsTab(name: string): Promise<void> {
  await goDashboard();
  await $(`button[aria-label="Télémétrie de ${name}"]`).click();
  await $('button[aria-label="Plein écran"]').click();
  await waitTelemetryTab(name);
}

/** Ferme tous les onglets ; une commande en cours est interrompue (GO). */
async function closeAllTabs(): Promise<void> {
  await dismissToasts();
  for (const name of await tabNames()) {
    await $(`button[aria-label="Fermer l'onglet ${name}"]`).click();
    if (
      await $(DIALOG)
        .isDisplayed()
        .catch(() => false)
    )
      await go();
  }
  await browser.waitUntil(async () => (await tabNames()).length === 0);
}

describe("Jalon 11 · 11. Limites des onglets et des flux", () => {
  before(async () => {
    seed();
    await connect();
  });

  afterEach(async () => {
    await dismissToasts();
  });

  it("4 flux : 3 onglets de logs et un up long, la commande suivante est refusée", async () => {
    for (const name of ["nominal-web-1", "nominal-app-1", "kuma"])
      await openLogsTab(name);

    sh("cd /tmp && docker compose -p slow down -t 0");
    await goDashboard();
    await refresh();
    await waitProjectState("slow", /^DESCENDU/i);
    await runProjectAction("slow", "Démarrer");
    await waitTelemetryTab("up · slow");

    await goDashboard();
    await runProjectAction("partial", "Redémarrer");
    const toast = await waitToast("Commande non lancée");
    expect(await textOf(toast.$(".message"))).toBe(
      "4 flux déjà ouverts : fermez un onglet de télémétrie pour lancer cette commande",
    );
    await browser.pause(500);
    expect(await tabNames()).not.toContain("restart · partial");

    await goTelemetry();
    await closeAllTabs();
  });

  it("6 onglets compose au plus : le plus ancien terminé sort, les logs ne comptent pas", async () => {
    await openLogsTab("kuma");
    const projects = [
      "nominal",
      "partial",
      "failed",
      "oom",
      "loop",
      "multi",
      "envfile",
    ];
    for (const name of projects) {
      await goDashboard();
      await runProjectAction(name, "Récupérer les images");
      await waitTelemetryTab(`pull · ${name}`);
      await waitConsoleEnd();
    }
    const names = await tabNames();
    const compose = names.filter((name) => COMPOSE_TAB.test(name));
    expect(compose).toEqual(projects.slice(1).map((name) => `pull · ${name}`));
    expect(names).toContain("kuma");
  });

  it("Relancer la même commande remplace son onglet terminé", async () => {
    await waitTelemetryTab("pull · envfile");
    await button("Relancer").click();
    await waitConsoleEnd();
    await browser.pause(500);
    expect(
      (await tabNames()).filter((name) => name === "pull · envfile"),
    ).toHaveLength(1);
  });
});
