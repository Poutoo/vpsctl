/** docs/tests-jalon-11.md, section 8 : Arrêter et supprimer (GO / NO GO du down). */
import { $, browser, expect } from "@wdio/globals";
import { TARGET } from "../../support/env.ts";
import { projectContainers, seed, sh, volumes } from "../../support/target.ts";
import {
  connect,
  DIALOG,
  dialogItems,
  go,
  goDashboard,
  noGo,
  runProjectAction,
  textOf,
  waitConsoleEnd,
  waitDialog,
  waitProjectState,
  waitTelemetryTab,
} from "../../support/ui.ts";

const DOWN_WARNING =
  "Les conteneurs seront recréés par Démarrer et les volumes nommés sont conservés, mais les données écrites hors des volumes nommés sont perdues.";

function anonymousVolume(): string {
  const name = sh(
    "docker inspect anon-db-1 --format '{{range .Mounts}}{{if eq .Destination \"/cache\"}}{{.Name}}{{end}}{{end}}'",
  ).trim();
  expect(name).toMatch(/^[0-9a-f]{64}$/);
  return name;
}

describe("Jalon 11 · 8. Arrêter et supprimer", () => {
  before(async () => {
    seed();
    await connect();
  });

  it("GO / NO GO sans volume anonyme (nominal), focus sur NO GO, Échap = NO GO", async () => {
    await runProjectAction("nominal", "Arrêter et supprimer");
    await waitDialog();

    expect(await textOf(`${DIALOG} h2`)).toMatch(
      /^Arrêter et supprimer les conteneurs du projet \?$/i,
    );
    expect(await textOf(`${DIALOG} [data-testid="confirm-subject"]`)).toBe(
      `nominal · ${TARGET.tests}/nominal`,
    );
    expect(await textOf(`${DIALOG} [data-testid="confirm-command"]`)).toBe(
      "docker compose --ansi never --progress plain -p 'nominal' down",
    );
    const aside = await textOf(`${DIALOG} [data-testid="confirm-aside"]`);
    expect(aside).toMatch(/^DONNÉES HORS VOLUMES PERDUES$/i);
    expect(aside).not.toMatch(/Réversible/i);

    expect(await dialogItems("confirm-removal")).toEqual([
      "2 conteneurs arrêtés puis supprimés · 2 en marche | app · web",
      "Réseau du projet supprimé | nominal_default",
    ]);
    expect(await dialogItems("confirm-check")).toEqual([
      "Volumes nommés conservés · down sans --volumes | nominal_data",
      "Images conservées · down sans --rmi",
    ]);
    expect(await dialogItems("confirm-consequence")).toEqual([
      "Relance possible avec Démarrer, le fichier compose est présent : compose.yaml.",
    ]);
    expect(await textOf(`${DIALOG} [data-testid="confirm-warning"]`)).toBe(
      DOWN_WARNING,
    );

    const focused = await browser.execute(
      () => document.activeElement?.textContent ?? "",
    );
    expect(focused.replace(/\s+/g, " ").trim()).toBe("NO GO Annuler");

    await browser.keys("Escape");
    await $(DIALOG).waitForDisplayed({ reverse: true });
    expect(projectContainers("nominal")).toEqual([
      "nominal-app-1",
      "nominal-web-1",
    ]);
  });

  it("GO / NO GO avec volume anonyme (anon) : volume détaché, dossier monté conservé", async () => {
    const anonymous = anonymousVolume();
    await goDashboard();
    await runProjectAction("anon", "Arrêter et supprimer");
    await waitDialog();

    expect(await dialogItems("confirm-removal")).toContain(
      `1 volume anonyme détaché : conservé sur le serveur, mais plus rattaché au projet après le down | ${anonymous.slice(0, 12)}`,
    );
    const checks = await dialogItems("confirm-check");
    expect(checks).toContain(
      "Volumes nommés conservés · down sans --volumes | anon_named",
    );
    expect(checks).toContain(
      `Dossiers du serveur montés conservés | ${TARGET.tests}/anon/uploads`,
    );
    await noGo();
  });

  it("après le down de anon : volumes conservés, aucun conteneur, Descendu · 0/1", async () => {
    const anonymous = anonymousVolume();
    await runProjectAction("anon", "Arrêter et supprimer");
    await waitDialog();
    await go();
    await waitTelemetryTab("down · anon");
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);

    const all = volumes();
    expect(all).toContain("anon_named");
    expect(all).toContain(anonymous);
    expect(projectContainers("anon")).toEqual([]);
    await goDashboard();
    await waitProjectState("anon", /^DESCENDU · 0\/1$/i);
  });

  it("service retiré du fichier (multi) : aucun conteneur orphelin après le down", async () => {
    sh("printf 'services: {}\\n' > ~/atlas-tests/multi/compose.prod.yaml");
    await runProjectAction("multi", "Arrêter et supprimer");
    await waitDialog();
    await go();
    await waitTelemetryTab("down · multi");
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);
    expect(projectContainers("multi")).toEqual([]);
  });
});
