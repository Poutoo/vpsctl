/** docs/tests-jalon-11.md, section 3 : détection du plugin compose. */
import { $, browser, expect } from "@wdio/globals";
import { PROFILE } from "../../support/appConfig.ts";
import {
  hideComposePlugin,
  restoreComposePlugin,
  seed,
  sh,
} from "../../support/target.ts";
import {
  connect,
  container,
  disconnect,
  project,
  projectAction,
  statusSummary,
  textOf,
} from "../../support/ui.ts";

async function waitSummary(pattern: RegExp): Promise<string> {
  let summary = "";
  await browser.waitUntil(
    async () => pattern.test((summary = await statusSummary())),
    {
      timeout: 30_000,
      timeoutMsg: `barre d'état « ${summary} », attendu ${pattern}`,
    },
  );
  return summary;
}

describe("Jalon 11 · 3. Détection du plugin", () => {
  before(async () => {
    seed();
    await connect();
  });

  after(() => restoreComposePlugin());

  it("barre d'état : système · Docker · Compose · systemd", async () => {
    const os = sh(". /etc/os-release; echo $PRETTY_NAME").trim();
    const docker = sh("docker version --format '{{.Server.Version}}'").trim();
    const systemd = /systemd (\d+)/.exec(sh("systemctl --version"))?.[1];
    expect(docker).toBe("29.6.1");
    expect(await waitSummary(/Compose/)).toBe(
      `${os} · Docker ${docker} · Compose 5.3.1 · systemd ${systemd}`,
    );
  });

  it("plugin absent : « Compose absent », avis, groupes sans actions de projet", async () => {
    hideComposePlugin();
    await disconnect();
    await connect();

    expect(await waitSummary(/Compose absent/)).toMatch(/· Compose absent ·/);
    const notice = $('[data-testid="compose-absent"]');
    await notice.waitForDisplayed();
    expect(await textOf(notice.$(".title"))).toBe(
      `Le plugin docker compose n'est pas installé sur ${PROFILE.name}.`,
    );

    await project("nominal").waitForExist();
    for (const label of [
      "Démarrer",
      "Récupérer les images",
      "Redémarrer",
      "Arrêter et supprimer",
    ]) {
      expect(await projectAction("nominal", label).isExisting()).toBe(false);
    }
    const stop = container("nominal-web-1").$(
      'button[aria-label^="Arrêter nominal-web-1"]',
    );
    expect(await stop.isExisting()).toBe(true);
    expect(await stop.getAttribute("aria-disabled")).toBeNull();
  });

  it("plugin remis en place : actions de projet de retour", async () => {
    restoreComposePlugin();
    await disconnect();
    await connect();
    await waitSummary(/Compose 5\.3\.1/);
    expect(await $('[data-testid="compose-absent"]').isExisting()).toBe(false);
    await projectAction("nominal", "Démarrer").waitForExist();
  });
});
