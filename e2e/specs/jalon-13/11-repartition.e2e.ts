/**
 * docs/tests-jalon-13.md, section 11 : répartition de l'espace Docker, à
 * l'alerte du point de montage de la racine Docker. Variante automatique
 * (`DOCKER_USAGE_AUTOMATIC`, valeur du build) ; la variante à la demande
 * passe par le même mécanisme que l'analyse des volumes (section 12).
 */
import { $, browser, expect } from "@wdio/globals";
import {
  DOCKER_MOUNT,
  diskRow,
  goResources,
  sampleProcesses,
  seedResources,
  setThreshold,
  waitCpuValue,
} from "../../support/resources.ts";
import { connect, textOf } from "../../support/ui.ts";

const SPACE = '[data-testid="docker-space"]';

describe("Jalon 13 · 11. Répartition Docker", () => {
  before(async () => {
    seedResources();
    await connect();
    await goResources();
    await waitCpuValue();
  });

  it("alerte sur le point de montage de Docker : répartition par catégorie", async () => {
    expect(await $(SPACE).isExisting()).toBe(false);
    await setThreshold("disk", "attention", "1");
    await $(SPACE).waitForDisplayed();
    await $('[data-testid="docker-category-images"]').waitForDisplayed({
      timeout: 60_000,
    });
    const text = await textOf(SPACE);
    expect(text).toMatch(
      /^ESPACE DOCKER · PAR CATÉGORIE [\d, ]+ [KMGT]o utilisés sur [\d, ]+ [KMGT]o/i,
    );
    for (const label of ["Images", "Conteneurs", "Volumes", "Cache de build"])
      expect(text).toContain(label);
    expect(text).toMatch(/Répartition de \d{2}:\d{2}:\d{2}/i);
    expect(await textOf('[data-testid="volumes-analyze"]')).toBe(
      "Analyser les volumes",
    );
    // La cause est sous la ligne du point de montage de Docker.
    const below = await browser.execute(
      (row: string, space: string) => {
        const a = document.querySelector(row);
        const b = document.querySelector(space);
        return !!a && !!b && a.nextElementSibling === b;
      },
      diskRow(DOCKER_MOUNT),
      SPACE,
    );
    expect(below).toBe(true);
  });

  it("pas plus d'une répartition toutes les 5 minutes", async () => {
    // Un relevé toutes les 5 s pendant 40 s : au plus une exécution, déjà faite.
    const seen = await sampleProcesses("docker system df --format", 40_000);
    expect(seen.size).toBe(0);
  });

  it("alerte retombée : la répartition disparaît", async () => {
    await setThreshold("disk", "attention", "80");
    await browser.waitUntil(async () => !(await $(SPACE).isExisting()));
  });
});
