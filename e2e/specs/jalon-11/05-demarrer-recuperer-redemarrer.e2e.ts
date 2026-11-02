/** docs/tests-jalon-11.md, section 5 : Démarrer, Récupérer, Redémarrer. */
import { $, browser, expect } from "@wdio/globals";
import { TARGET } from "../../support/env.ts";
import { seed, sh } from "../../support/target.ts";
import {
  colorOf,
  connect,
  consoleCommand,
  consoleLines,
  consoleWhere,
  container,
  containerState,
  dismissToasts,
  goDashboard,
  invokeCalls,
  project,
  projectAction,
  projectDetail,
  projectState,
  recordInvokes,
  refresh,
  runProjectAction,
  softChecks,
  textOf,
  tokenColor,
  unavailableReason,
  waitConsoleEnd,
  waitProjectState,
  waitTelemetryTab,
  waitToast,
} from "../../support/ui.ts";

const PREFIX = "docker compose --ansi never --progress plain";
const dir = (name: string) => `${TARGET.tests}/${name}`;

/** Projet `failed` dont l'image de `job` n'existe pas. */
const FAILED_PULL = `services:
  web:
    image: busybox:1.37
    command: sleep 1d
    stop_grace_period: 1s
  job:
    image: ghcr.io/atlas-test/inexistant:latest
    command: sh -c "exit 3"
`;

describe("Jalon 11 · 5. Démarrer, Récupérer, Redémarrer", () => {
  before(async () => {
    seed();
    await connect();
  });

  afterEach(async () => {
    await dismissToasts();
  });

  it("infobulle de Redémarrer : libellé en gras, puis l'explication", async () => {
    await projectAction("nominal", "Redémarrer").moveTo();
    const tip = $(".app-tooltip");
    await tip.waitForDisplayed();
    expect(await textOf(tip.$("strong"))).toBe("Redémarrer le projet nominal");
    expect(await textOf(tip.$(".app-tooltip-detail"))).toBe(
      "Relance les conteneurs sans appliquer les changements du fichier compose ni les nouvelles images. Pour les appliquer : Démarrer.",
    );
    expect(await colorOf(tip.$("strong"), "font-weight")).toMatch(
      /^(600|700|bold)$/,
    );
    await $("h1").moveTo();
  });

  it("Démarrer sur partial : onglet, commande exacte, lignes err, exit 0, notification", async () => {
    await runProjectAction("partial", "Démarrer");
    await waitTelemetryTab("up · partial");
    expect(await textOf('[data-testid="page-surtitle"]')).toMatch(
      /^TÉLÉMÉTRIE · COMMANDES COMPOSE$/i,
    );
    expect(await consoleCommand()).toBe(
      `${PREFIX} -p 'partial' --project-directory '${dir("partial")}' -f '${dir("partial")}/compose.yaml' up -d`,
    );
    expect(await consoleWhere()).toBe(`dans ${dir("partial")}`);
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);
    const lines = await consoleLines();
    expect(lines.length).toBeGreaterThan(0);
    expect(lines.some((line) => line.origin === "err")).toBe(true);

    const toast = await waitToast("Projet démarré");
    expect(await textOf(toast.$(".detail"))).toMatch(/^partial · exit 0 · .+/);
  });

  it("seul le code de sortie juge : sortie toute sur err, notée en succès", async () => {
    await goDashboard();
    await runProjectAction("nominal", "Récupérer les images");
    await waitTelemetryTab("pull · nominal");
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);
    const lines = await consoleLines();
    expect(lines.length).toBeGreaterThan(0);
    expect(lines.every((line) => line.origin === "err")).toBe(true);
    await waitToast("Images récupérées");
  });

  it("up long avec téléchargement : chronos, progression en texte simple, pas d'arrêt à 30 s", async () => {
    const checks = softChecks();
    // Image de slow-web retirée : up la télécharge, puis attend 40 s que db soit sain.
    sh("docker rmi -f busybox:1.36");
    await goDashboard();
    const started = Date.now();
    await runProjectAction("slow", "Démarrer");
    await waitTelemetryTab("up · slow");

    const tab = await textOf('[data-testid="telemetry-tab"].active');
    checks.check("onglet avec chrono T+ (maquette 02)", /T\+\d/.test(tab), tab);
    const runningLabel = $('[data-testid="console-running"]');
    const header = (await runningLabel.isExisting())
      ? await textOf(runningLabel)
      : "";
    checks.check(
      "console « EN COURS · T+… » (maquette 02)",
      /^EN COURS · T\+\d/i.test(header),
      header,
    );

    await browser.waitUntil(
      async () =>
        (await consoleLines()).some((l) => /busybox|Pull/i.test(l.text)),
      { timeout: 30_000, timeoutMsg: "aucune ligne de téléchargement" },
    );
    for (const line of await consoleLines()) {
      checks.check(
        "ligne sans code de terminal ni retour chariot",
        !/[\u001b\r]/.test(line.text),
        line.text,
      );
    }

    await goDashboard();
    const state = await projectState("slow");
    checks.check("en-tête « UP · T+… »", /^UP · T\+/i.test(state), state);
    const detail = await projectDetail("slow");
    checks.check(
      "détail « docker compose up · en cours »",
      detail === "docker compose up · en cours",
      detail,
    );
    let web = "";
    await browser
      .waitUntil(
        async () =>
          /^DÉMARRAGE · T\+/i.test((web = await containerState("slow-web-1"))),
        {
          timeout: 30_000,
        },
      )
      .catch(() => undefined);
    checks.check(
      "slow-web-1 « DÉMARRAGE T+… »",
      /^DÉMARRAGE · T\+/i.test(web),
      web,
    );
    const webDetail = await textOf(
      container("slow-web-1").$('[data-testid="container-detail"]'),
    );
    checks.check(
      "slow-web-1 « via docker compose up »",
      webDetail === "via docker compose up",
      webDetail,
    );
    for (const label of [
      "Démarrer",
      "Récupérer les images",
      "Redémarrer",
      "Arrêter et supprimer",
    ]) {
      const reason = await unavailableReason(projectAction("slow", label));
      checks.check(`${label} du projet indisponible`, reason !== null);
    }
    const startReason = await unavailableReason(
      container("slow-web-1").$('button[aria-label^="Démarrer slow-web-1"]'),
    );
    checks.check(
      "Démarrer du conteneur indisponible",
      startReason === "commande compose en cours sur son projet",
      startReason,
    );
    const logsReason = await unavailableReason(
      container("slow-web-1").$(
        'button[aria-label^="Télémétrie de slow-web-1"]',
      ),
    );
    checks.check(
      "logs du conteneur disponibles",
      logsReason === null,
      logsReason,
    );
    const badge = await textOf('[data-testid="nav-compose-badge"]');
    checks.check("navigation « ◐ 1 »", /^1(\D|$)/.test(badge), badge);

    // Toujours en cours après 30 s : pas de délai d'abandon.
    await browser.pause(Math.max(0, started + 33_000 - Date.now()));
    const later = await projectState("slow");
    checks.check(
      "toujours en cours après 30 s",
      /^UP · T\+/i.test(later),
      later,
    );

    await browser.waitUntil(
      async () => (await projectDetail("slow")) === "compose.yaml",
      {
        timeout: 90_000,
        timeoutMsg: "up de slow jamais terminé",
      },
    );
    const running = sh(
      "docker inspect -f '{{.State.Running}}' slow-web-1",
    ).trim();
    checks.check("slow-web-1 démarré à la fin", running === "true", running);
    await waitToast("Projet démarré");
    checks.assertAll();
  });

  it("échec d'un pull : exit 1, notification persistante, détail rouge qui mène à la sortie", async () => {
    sh(`cat > ~/atlas-tests/failed/compose.yaml <<'EOF'\n${FAILED_PULL}EOF`);
    await goDashboard();
    await runProjectAction("failed", "Récupérer les images");
    await waitTelemetryTab("pull · failed");
    const banner = await waitConsoleEnd();
    expect(banner).toMatch(/exit 1/i);
    expect(await colorOf($('[data-testid="console-los"]'))).toBe(
      await tokenColor("--color-danger-text"),
    );

    const toast = await waitToast("Échec de docker compose pull");
    expect(await textOf(toast.$(".detail"))).toMatch(/^failed · exit 1/);
    expect(await toast.$("button.action").getText()).toMatch(/Voir la sortie/);
    await browser.pause(6_000);
    expect(await toast.isDisplayed()).toBe(true);

    await goDashboard();
    const detail = project("failed").$('[data-testid="project-detail"]');
    expect(await textOf(detail)).toBe("pull · exit 1 · voir");
    expect(await colorOf(detail.$("button.outcome"))).toBe(
      await tokenColor("--color-danger-text"),
    );
    await detail.$("button.outcome").click();
    await waitTelemetryTab("pull · failed");

    // La marque disparaît au lancement de la commande suivante sur ce projet.
    seed("failed");
    await goDashboard();
    await runProjectAction("failed", "Redémarrer");
    await waitTelemetryTab("restart · failed");
    await waitConsoleEnd();
    await goDashboard();
    expect(await projectDetail("failed")).toBe("compose.yaml");
  });

  it("Redémarrer sur nominal : par le nom du projet, sans -f ni --project-directory", async () => {
    await goDashboard();
    await runProjectAction("nominal", "Redémarrer");
    await waitTelemetryTab("restart · nominal");
    expect(await consoleCommand()).toBe(`${PREFIX} -p 'nominal' restart`);
    expect(await consoleWhere()).toBe("par le nom du projet");
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);
  });

  it("une commande à la fois par projet ; une autre part normalement", async () => {
    // Recréé depuis l'état descendu : db repart pour 40 s avant d'être sain.
    sh("cd /tmp && docker compose -p slow down -t 0");
    await goDashboard();
    await refresh();
    await waitProjectState("slow", /^DESCENDU/i);
    await runProjectAction("slow", "Démarrer");
    await waitTelemetryTab("up · slow");

    await goDashboard();
    expect(
      await unavailableReason(projectAction("slow", "Récupérer les images")),
    ).toBe("commande compose en cours sur ce projet");
    await runProjectAction("profiled", "Redémarrer");
    await waitTelemetryTab("restart · profiled");
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);

    await goDashboard();
    await waitProjectState("slow", /^NOMINAL · 2\/2$/i, 120_000);
  });

  it("après chaque commande, la liste et la vérification des fichiers sont relues", async () => {
    await goDashboard();
    await runProjectAction("nominal", "Redémarrer");
    await waitTelemetryTab("restart · nominal");
    // Hors du tableau de bord, rien n'est interrogé périodiquement.
    await recordInvokes();
    await waitConsoleEnd();
    await browser.waitUntil(
      async () => {
        const calls = await invokeCalls();
        return calls.includes("docker_list") && calls.includes("compose_check");
      },
      {
        timeout: 15_000,
        timeoutMsg: "liste ou vérification non relue après la commande",
      },
    );
  });
});
