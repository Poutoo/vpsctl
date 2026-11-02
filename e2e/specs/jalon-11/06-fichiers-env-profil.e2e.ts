/** docs/tests-jalon-11.md, section 6 : plusieurs fichiers, fichier d'environnement, profil. */
import { expect } from "@wdio/globals";
import { TARGET } from "../../support/env.ts";
import { seed, sh } from "../../support/target.ts";
import {
  connect,
  consoleCommand,
  goDashboard,
  project,
  projectDetail,
  runProjectAction,
  waitConsoleEnd,
  waitTelemetryTab,
} from "../../support/ui.ts";

const dir = (name: string) => `${TARGET.tests}/${name}`;

function containerId(name: string): string {
  return sh(`docker inspect -f '{{.Id}}' ${name}`).trim();
}

describe("Jalon 11 · 6. Plusieurs fichiers, fichier d'environnement, profil", () => {
  before(async () => {
    seed();
    await connect();
  });

  it("multi : deux fichiers, dans l'ordre, worker reste créé", async () => {
    expect(await projectDetail("multi")).toBe(
      "compose.yaml + compose.prod.yaml",
    );
    await runProjectAction("multi", "Démarrer");
    await waitTelemetryTab("up · multi");
    expect(await consoleCommand()).toContain(
      `-f '${dir("multi")}/compose.yaml' -f '${dir("multi")}/compose.prod.yaml' up -d`,
    );
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);
    expect(
      sh("docker inspect -f '{{.State.Running}}' multi-worker-1").trim(),
    ).toBe("true");
  });

  it("envfile : --env-file transmis, sleep 1d conservé, ligne --env-file en infobulle", async () => {
    await goDashboard();
    expect(
      await project("envfile")
        .$('[data-testid="project-detail"]')
        .getAttribute("title"),
    ).toBe(
      `-f ${dir("envfile")}/compose.yaml\n--env-file ${dir("envfile")}/.env.prod`,
    );

    await runProjectAction("envfile", "Démarrer");
    await waitTelemetryTab("up · envfile");
    expect(await consoleCommand()).toContain(
      `--env-file '${dir("envfile")}/.env.prod'`,
    );
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);
    expect(
      sh("docker inspect -f '{{json .Config.Cmd}}' envfile-web-1").trim(),
    ).toBe('["sleep","1d"]');
  });

  // Limite documentée : le profil n'est enregistré dans aucune étiquette.
  it("profiled : Démarrer ne lance que web, debug n'est ni recréé ni arrêté", async () => {
    const debugBefore = containerId("profiled-debug-1");
    await goDashboard();
    await runProjectAction("profiled", "Démarrer");
    await waitTelemetryTab("up · profiled");
    expect(await consoleCommand()).not.toContain("--profile");
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);

    expect(containerId("profiled-debug-1")).toBe(debugBefore);
    expect(
      sh(
        "docker inspect -f '{{.State.Running}}' profiled-debug-1 profiled-web-1",
      ).trim(),
    ).toBe("true\ntrue");
  });
});
