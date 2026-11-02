/** docs/tests-jalon-11.md, section 7 : dossier supprimé, fichier illisible. */
import { browser, expect } from "@wdio/globals";
import { TARGET } from "../../support/env.ts";
import { projectContainers, seed, sh } from "../../support/target.ts";
import {
  colorOf,
  connect,
  consoleCommand,
  dialogItems,
  dismissToasts,
  go,
  goDashboard,
  invokeCalls,
  project,
  projectAction,
  projectDetail,
  recordInvokes,
  refresh,
  runProjectAction,
  tabNames,
  textOf,
  unavailableReason,
  waitConsoleEnd,
  waitDialog,
  waitProjectState,
  waitTelemetryTab,
  waitToast,
} from "../../support/ui.ts";

const NOMINAL = `${TARGET.tests}/nominal`;
const ENV_FILE = `${TARGET.tests}/envfile/.env.prod`;

/** Attend un détail de projet, en actualisant (Actualiser relance aussi la vérification). */
async function waitDetail(name: string, expected: string): Promise<void> {
  let last = "";
  await browser.waitUntil(
    async () => {
      await refresh();
      return (last = await projectDetail(name)) === expected;
    },
    {
      timeout: 45_000,
      interval: 2_000,
      timeoutMsg: `détail de ${name} : « ${last} »`,
    },
  );
}

describe("Jalon 11 · 7. Dossier supprimé, fichier illisible", () => {
  before(async () => {
    seed();
    await connect();
  });

  after(() => seed("nominal", "envfile"));

  it("dossier supprimé : en-tête teinté, chemin barré, explication, actions selon la raison", async () => {
    sh(`mv ${NOMINAL} ${NOMINAL}.bak`);
    await waitDetail("nominal", "dossier introuvable");

    const row = project("nominal");
    // En-tête teinté : fond différent de celui d'un projet sans problème.
    expect(await colorOf(row, "background-color")).not.toBe(
      await colorOf(project("multi"), "background-color"),
    );
    const dir = row.$('[data-testid="project-dir"]');
    expect(
      await dir.$('[data-testid="icon-folder-missing"]').isExisting(),
    ).toBe(true);
    expect(await colorOf(dir.$(".dir-path"), "text-decoration-line")).toBe(
      "line-through",
    );
    expect(
      await project("nominal")
        .$('[data-testid="project-detail"] [data-testid="shape-triangle"]')
        .isExisting(),
    ).toBe(true);
    expect(await textOf('[data-testid="project-problem-nominal"]')).toContain(
      `${NOMINAL} n'existe plus sur le serveur.`,
    );

    expect(await unavailableReason(projectAction("nominal", "Démarrer"))).toBe(
      "dossier introuvable",
    );
    expect(
      await unavailableReason(projectAction("nominal", "Récupérer les images")),
    ).toBe("dossier introuvable");
    expect(
      await unavailableReason(projectAction("nominal", "Redémarrer")),
    ).toBeNull();
    expect(
      await unavailableReason(projectAction("nominal", "Arrêter et supprimer")),
    ).toBeNull();
  });

  it("Redémarrer sur ce projet réussit, par le nom du projet", async () => {
    await runProjectAction("nominal", "Redémarrer");
    await waitTelemetryTab("restart · nominal");
    expect(await consoleCommand()).toMatch(/ -p 'nominal' restart$/);
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);
  });

  it("Arrêter et supprimer : relance impossible annoncée, puis Descendu · 0/2", async () => {
    await goDashboard();
    await runProjectAction("nominal", "Arrêter et supprimer");
    await waitDialog();
    expect(await dialogItems("confirm-consequence")).toContain(
      `Relance impossible depuis Atlas : ${NOMINAL} n'existe plus sur le serveur.`,
    );
    await go();
    await waitTelemetryTab("down · nominal");
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);
    expect(projectContainers("nominal")).toEqual([]);
    expect(sh("docker network ls --format '{{.Name}}'")).not.toContain(
      "nominal_default",
    );

    await goDashboard();
    await waitProjectState("nominal", /^DESCENDU · 0\/2$/i);

    sh(`mv ${NOMINAL}.bak ${NOMINAL}`);
    await waitDetail("nominal", "aucun conteneur · mémorisé");
    await runProjectAction("nominal", "Démarrer");
    await waitTelemetryTab("up · nominal");
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);
    await goDashboard();
    await waitProjectState("nominal", /^NOMINAL · 2\/2$/i);
  });

  it("fichier illisible : détail, explication, Démarrer et Récupérer indisponibles", async () => {
    sh(`chmod 000 ${ENV_FILE}`);
    await waitDetail("envfile", "fichier illisible");
    expect(await textOf('[data-testid="project-problem-envfile"]')).toContain(
      `${ENV_FILE} est illisible pour le compte connecté.`,
    );
    expect(await unavailableReason(projectAction("envfile", "Démarrer"))).toBe(
      "fichier illisible",
    );
    expect(
      await unavailableReason(projectAction("envfile", "Récupérer les images")),
    ).toBe("fichier illisible");
    sh(`chmod 644 ${ENV_FILE}`);
    await waitDetail("envfile", "compose.yaml");
  });

  it("fichier rendu illisible juste avant Démarrer : refus, aucun onglet ne reste", async () => {
    // Actualiser vient de vérifier les fichiers : la suivante est dans 30 s.
    await refresh();
    sh(`chmod 000 ${ENV_FILE}`);
    await runProjectAction("envfile", "Démarrer");
    const toast = await waitToast("Commande non lancée");
    expect(await textOf(toast.$(".message"))).toContain(
      `fichier illisible pour le compte connecté : ${ENV_FILE}`,
    );
    await browser.pause(1_000);
    expect(await tabNames()).not.toContain("up · envfile");
    sh(`chmod 644 ${ENV_FILE}`);
    await dismissToasts();
  });

  it("la vérification des fichiers n'a pas lieu à chaque actualisation de 5 s", async () => {
    await goDashboard();
    await browser.pause(1_000);
    await recordInvokes();
    await browser.pause(21_000);
    const calls = await invokeCalls();
    const lists = calls.filter((c) => c === "docker_list").length;
    const checks = calls.filter((c) => c === "compose_check").length;
    expect(lists).toBeGreaterThanOrEqual(3);
    expect(checks).toBeLessThanOrEqual(1);
  });
});
