/** docs/tests-jalon-11.md, section 4 : groupes et états. */
import { $, $$, browser, expect } from "@wdio/globals";
import { TARGET } from "../../support/env.ts";
import { seed, sh } from "../../support/target.ts";
import {
  colorOf,
  connect,
  container,
  containerState,
  DOCKER,
  goDashboard,
  goScripts,
  project,
  projectDetail,
  projectShape,
  projectState,
  refresh,
  runProjectAction,
  setFilter,
  textOf,
  tokenColor,
  waitConsoleEnd,
  waitProjectState,
  waitTelemetryTab,
} from "../../support/ui.ts";

async function shownProjects(): Promise<string[]> {
  const rows = Array.from(await $$(`${DOCKER} tr.project-row`));
  return Promise.all(
    rows.map(async (row) =>
      ((await row.getAttribute("data-testid")) ?? "").replace("project-", ""),
    ),
  );
}

async function shownContainers(): Promise<string[]> {
  const rows = Array.from(await $$(`${DOCKER} tr[data-testid^="container-"]`));
  return Promise.all(
    rows.map(async (row) =>
      ((await row.getAttribute("data-testid")) ?? "").replace("container-", ""),
    ),
  );
}

describe("Jalon 11 · 4. Groupes et états", () => {
  before(async () => {
    seed();
    await connect();
  });

  after(async () => {
    await setFilter("").catch(() => undefined);
  });

  it("décompte, projets triés par nom, puis « Hors compose » en dernier", async () => {
    const projects = sh(
      "docker ps -a --format '{{.Label \"com.docker.compose.project\"}}' | sort -u",
    )
      .split("\n")
      .filter(Boolean);
    expect(await textOf(`${DOCKER} [data-testid="section-command"]`)).toBe(
      `docker ps -a · ${projects.length} projets compose · 1 hors compose`,
    );
    expect(await shownProjects()).toEqual([...projects].sort());

    const heads = Array.from(await $$(`${DOCKER} tbody.group-head`));
    const last = heads.at(-1);
    expect(await last?.$('[data-testid="loose-group"]').isExisting()).toBe(
      true,
    );
    expect(await textOf($('[data-testid="loose-group"]'))).toMatch(
      /^Hors compose ?1 CONTENEUR · sans étiquette com\.docker\.compose\.project$/i,
    );
    expect(
      await colorOf(
        $('[data-testid="loose-group"] .loose-title'),
        "font-style",
      ),
    ).toBe("italic");
  });

  it("projet nominal : NOMINAL · 2/2, disque, dossier, détail compose.yaml", async () => {
    expect(await projectState("nominal")).toMatch(/^NOMINAL · 2\/2$/i);
    expect(await projectShape("nominal")).toBe("disc");
    const dir = project("nominal").$('[data-testid="project-dir"]');
    expect(await textOf(dir)).toBe(`${TARGET.tests}/nominal`);
    expect(await dir.$('[data-testid="icon-folder"]').isExisting()).toBe(true);
    expect(await projectDetail("nominal")).toBe("compose.yaml");
    expect(
      await project("nominal")
        .$('[data-testid="project-detail"]')
        .getAttribute("title"),
    ).toBe(`-f ${TARGET.tests}/nominal/compose.yaml`);
  });

  it("projet partiel : PARTIEL · 1/2, cible, ambre ; migrate ARRÊTÉ · code 0", async () => {
    expect(await projectState("partial")).toMatch(/^PARTIEL · 1\/2$/i);
    expect(await projectShape("partial")).toBe("target");
    const shape = project("partial").$(
      '[data-testid="project-state"] [data-testid="shape-target"]',
    );
    expect(await colorOf(shape)).toBe(await tokenColor("--color-warning"));
    expect(await containerState("partial-migrate-1")).toMatch(
      /^ARRÊTÉ · CODE 0$/i,
    );
  });

  it("tous arrêtés proprement : ARRÊTÉ · 0/2, cercle vide, puis Démarrer", async () => {
    sh("cd /tmp && docker compose -p nominal stop");
    await refresh();
    await waitProjectState("nominal", /^ARRÊTÉ · 0\/2$/i);
    expect(await projectShape("nominal")).toBe("circle");

    await runProjectAction("nominal", "Démarrer");
    await waitTelemetryTab("up · nominal");
    expect(await waitConsoleEnd()).toMatch(/exit 0/i);
    await goDashboard();
    await waitProjectState("nominal", /^NOMINAL · 2\/2$/i);
  });

  it("projet en échec : EN ÉCHEC · 1/2, triangle, en-tête teinté ; job code 3", async () => {
    expect(await projectState("failed")).toMatch(/^EN ÉCHEC · 1\/2$/i);
    expect(await projectShape("failed")).toBe("triangle");
    expect(await colorOf(project("failed"), "background-color")).not.toBe(
      await colorOf(project("nominal"), "background-color"),
    );
    expect(await containerState("failed-job-1")).toMatch(
      /^EN ÉCHEC · CODE 3$/i,
    );
  });

  it("manque de mémoire : oom EN ÉCHEC, hog porte MÉMOIRE et la limite en infobulle", async () => {
    expect(await projectState("oom")).toMatch(/^EN ÉCHEC/i);
    expect(sh("docker inspect -f '{{.State.ExitCode}}' oom-hog-1").trim()).toBe(
      "137",
    );
    expect(await containerState("oom-hog-1")).toMatch(/MÉMOIRE/i);
    const raw = await container("oom-hog-1")
      .$('[data-testid="container-state"] .run-state')
      .getAttribute("title");
    expect(raw).toMatch(/OOMKilled/);
    expect(raw).toMatch(/limite de 32 Mi?o/);
  });

  // Échantillonne l'état pendant 20 s : le projet doit rester « En échec »
  // à chaque actualisation, y compris quand `crash` est entre deux sorties.
  it("boucle de redémarrage : loop EN ÉCHEC, sans passer par « En transition »", async () => {
    const seenUi = new Set<string>();
    const seenDocker = new Set<string>();
    const until = Date.now() + 20_000;
    while (Date.now() < until) {
      seenUi.add((await projectState("loop")).toUpperCase());
      seenDocker.add(
        sh(
          "docker inspect -f '{{.State.Status}} {{.State.ExitCode}}' loop-crash-1",
        ).trim(),
      );
      await browser.pause(500);
    }
    expect([...seenDocker]).toContain("restarting 1");
    for (const state of seenUi) expect(state).toMatch(/^EN ÉCHEC/);
  });

  it("un conteneur docker compose run apparaît avec RUN, décompte inchangé", async () => {
    // Nom par défaut de compose run, comme dans le test manuel.
    sh("cd ~/atlas-tests/nominal && docker compose run -d web sleep 600");
    try {
      await refresh();
      const row = $('tr[data-testid^="container-nominal-web-run-"]');
      await row.waitForExist({ timeout: 15_000 });
      const tag = row.$(".run-tag");
      expect(await tag.isExisting()).toBe(true);
      // La mention doit être lisible : entièrement dans la cellule du nom.
      const visible = await browser.execute(
        (element: HTMLElement) => {
          const cell = element.closest("td")!.getBoundingClientRect();
          const box = element.getBoundingClientRect();
          return box.left >= cell.left && box.right <= cell.right;
        },
        (await tag) as unknown as HTMLElement,
      );
      expect(visible).toBe(true);
      expect(await textOf(tag)).toMatch(/^RUN$/i);
      expect(await projectState("nominal")).toMatch(/^NOMINAL · 2\/2$/i);
    } finally {
      sh(
        "docker ps -aq --filter label=com.docker.compose.oneoff=True | xargs -r docker rm -f",
      );
      await refresh();
    }
  });

  it("repli : le chevron replie et déplie, le repli est gardé en changeant d'écran", async () => {
    await $('button[aria-label="Replier le projet nominal"]').click();
    await container("nominal-web-1").waitForExist({ reverse: true });

    await goScripts();
    await goDashboard();
    expect(await container("nominal-web-1").isExisting()).toBe(false);

    await $('button[aria-label="Déplier le projet nominal"]').click();
    await container("nominal-web-1").waitForExist();
  });

  it("filtre : un dossier montre son groupe entier, un nom seulement le conteneur", async () => {
    await setFilter("atlas-tests/multi");
    await browser.waitUntil(
      async () => (await shownProjects()).join() === "multi",
    );
    expect((await shownContainers()).sort()).toEqual([
      "multi-web-1",
      "multi-worker-1",
    ]);

    await setFilter("worker");
    await browser.waitUntil(
      async () => (await shownContainers()).join() === "multi-worker-1",
    );
    expect(await shownProjects()).toEqual(["multi"]);
    await setFilter("");
  });

  it("fenêtre étroite (1024 px) : colonne « État · Détail » sur deux lignes", async function () {
    try {
      await browser.setWindowSize(1024, 800);
    } catch (error) {
      console.warn(
        `[e2e] redimensionnement refusé par le pilote : ${String(error)}`,
      );
      this.skip();
    }
    try {
      await browser.waitUntil(
        async () => (await browser.execute(() => window.innerWidth)) <= 1100,
        { timeout: 5_000 },
      );
      expect(await textOf(`${DOCKER} thead th:nth-child(3)`)).toMatch(
        /^État · Détail$/i,
      );
      const stacked = project("nominal").$(
        '[data-testid="project-state"] .stacked-detail',
      );
      expect(await stacked.isDisplayed()).toBe(true);
      expect(await textOf(stacked)).toBe("compose.yaml");
      const label = project("nominal").$(
        '[data-testid="project-state"] .project-state',
      );
      expect((await stacked.getLocation()).y).toBeGreaterThan(
        (await label.getLocation()).y,
      );
    } finally {
      await browser.setWindowSize(1280, 800);
    }
  });
});
