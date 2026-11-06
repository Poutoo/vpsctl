/** docs/tests-jalon-11.md, section 10 : interruption, perte de liaison. */
import { $, browser, expect } from "@wdio/globals";
import {
  freezeTarget,
  seed,
  setSlowDelay,
  sh,
  shStatus,
  thawTarget,
} from "../../support/target.ts";
import {
  button,
  connect,
  dialogItems,
  DIALOG,
  go,
  goDashboard,
  noGo,
  projectDetail,
  refresh,
  runProjectAction,
  tabNames,
  textOf,
  waitConsoleEnd,
  waitDialog,
  waitDockerLoaded,
  waitProjectState,
  waitTelemetryTab,
} from "../../support/ui.ts";

const PARTIAL_START =
  "1 commande compose en cours sera interrompue : son projet peut rester à moitié démarré ou arrêté.";

/**
 * Lance un up de `slow` qui dure : db recréé, sain après `seconds`. 15 s
 * suffisent pour interrompre ou fermer pendant la commande ; la perte de
 * liaison en demande 40, l'up devant encore tourner à la relance qui suit
 * le gel de la cible (environ 25 s).
 */
async function startLongUp(seconds = 15): Promise<void> {
  setSlowDelay(seconds);
  sh("cd /tmp && docker compose -p slow down -t 0");
  await goDashboard();
  await refresh();
  await waitProjectState("slow", /^DESCENDU/i);
  await runProjectAction("slow", "Démarrer");
  await waitTelemetryTab("up · slow");
  await button("Interrompre le flux").waitForDisplayed();
}

describe("Jalon 11 · 10. Interruption, perte de liaison", () => {
  before(async () => {
    seed();
    await connect();
  });

  // Cible jamais laissée gelée, même si un test échoue pendant la coupure.
  after(() => {
    try {
      thawTarget();
    } catch {
      // Déjà dégelée.
    }
  });

  it("Interrompre un up en cours : GO / NO GO, NO GO continue, GO interrompt", async () => {
    await startLongUp();

    await button("Interrompre le flux").click();
    await waitDialog();
    expect(await textOf(`${DIALOG} h2`)).toMatch(
      /^Interrompre docker compose up \?$/i,
    );
    expect(await dialogItems("confirm-consequence")).toContain(
      "Le projet peut rester à moitié démarré : certains conteneurs créés ou démarrés, d'autres non. Démarrer termine le travail.",
    );
    await noGo();
    await browser.pause(1_000);
    expect(await button("Interrompre le flux").isDisplayed()).toBe(true);

    await button("Interrompre le flux").click();
    await waitDialog();
    await go();
    expect(await waitConsoleEnd(30_000)).toMatch(/Procédure interrompue/i);

    await goDashboard();
    await browser.waitUntil(
      async () => (await projectDetail("slow")) === "up · interrompue · voir",
      {
        timeout: 15_000,
      },
    );
  });

  it("fermer l'onglet d'une commande en cours : même GO / NO GO", async () => {
    await startLongUp();
    await $('button[aria-label="Fermer l\'onglet up · slow"]').click();
    await waitDialog();
    expect(await textOf(`${DIALOG} h2`)).toMatch(/^Fermer l'onglet \?$/i);
    expect(await dialogItems("confirm-consequence")).toContain(
      "docker compose up sera interrompu.",
    );
    await noGo();
    expect(await tabNames()).toContain("up · slow");
  });

  it("couper la liaison ou fermer l'application pendant une commande : GO / NO GO qui la compte", async () => {
    await button("Couper la liaison").click();
    await waitDialog();
    expect(await textOf(`${DIALOG} h2`)).toMatch(/^Couper la liaison \?$/i);
    expect(await dialogItems("confirm-consequence")).toContain(PARTIAL_START);
    await noGo();

    await $('button[aria-label="Fermer"]').click();
    await waitDialog();
    expect(await textOf(`${DIALOG} h2`)).toMatch(/^Fermer l'application \?$/i);
    expect(await dialogItems("confirm-consequence")).toContain(PARTIAL_START);
    await noGo();

    // Fin de la commande, pour repartir d'un état stable.
    await waitTelemetryTab("up · slow");
    await waitConsoleEnd(90_000);
  });

  it("perte de liaison pendant un up long : issue inconnue, rien relancé, Relancer après rétablissement", async () => {
    await startLongUp(40);
    freezeTarget();
    try {
      await $("p*=Liaison perdue avec").waitForDisplayed({ timeout: 45_000 });

      expect(
        await textOf(
          '[data-testid="telemetry-tab"].active [data-testid="tab-badge"]',
        ),
      ).toMatch(/^ISSUE INCONNUE$/i);
      expect(await textOf('[data-testid="console-lost"]')).toMatch(
        /^LIAISON PERDUE · ISSUE INCONNUE$/i,
      );
      expect(
        await $(
          '//li[@data-testid="toast"]//p[normalize-space()="Échec de docker compose up"]',
        ).isExisting(),
      ).toBe(false);
      // Pendant le LOS, Relancer est indisponible.
      expect(await button("Relancer").isEnabled()).toBe(false);

      await $(
        '//nav[@aria-label="Navigation principale"]//a[contains(., "Tableau de bord")]',
      ).click();
      expect(await projectDetail("slow")).toBe("up · issue inconnue · voir");
    } finally {
      thawTarget();
    }

    await button("Rétablir la liaison").click();
    await $("p*=Liaison perdue avec").waitForDisplayed({
      reverse: true,
      timeout: 60_000,
    });
    await waitDockerLoaded();
    expect(await $('[data-testid="nav-compose-badge"]').isExisting()).toBe(
      false,
    );

    // Sur le serveur, la commande a été arrêtée quand sshd a fermé le canal.
    await browser.waitUntil(
      () => shStatus("pgrep -f '[d]ocker compose' >/dev/null").code !== 0,
      {
        timeout: 30_000,
        timeoutMsg: "docker compose tourne encore sur le serveur",
      },
    );

    await $('[data-testid="project-slow"] button.outcome').click();
    await waitTelemetryTab("up · slow");
    expect(await button("Relancer").isEnabled()).toBe(true);
    await button("Relancer").click();
    // La relance remplace l'onglet terminé : la ligne de fin de l'exécution
    // interrompue peut rester affichée un instant après le clic.
    await $('[data-testid="console-los"]').waitForDisplayed({
      reverse: true,
      timeout: 15_000,
    });
    await button("Interrompre le flux").waitForDisplayed({ timeout: 15_000 });
    expect(await waitConsoleEnd(90_000)).toMatch(/exit 0/i);
  });
});
