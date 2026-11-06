/**
 * docs/tests-jalon-13.md, section 6 : mesure indisponible. CPU, mémoire,
 * charge et réseau restent manuels : le noyau fournit toujours leurs
 * fichiers de /proc (couverts par les tests Rust des analyseurs).
 */
import { $, expect } from "@wdio/globals";
import {
  diskRow,
  goResources,
  seedResources,
  surveyTime,
  waitCpuValue,
  waitNextSurvey,
} from "../../support/resources.ts";
import { sh } from "../../support/target.ts";
import { connect, textOf } from "../../support/ui.ts";

const LOCKED = "/srv/prive/disque";

describe("Jalon 13 · 6. Mesure indisponible", () => {
  before(async () => {
    seedResources();
    await connect();
    await goResources();
    await waitCpuValue();
  });

  after(() => seedResources());

  it("conteneurs : accès à Docker refusé, les autres mesures restent à jour", async () => {
    sh("chmod 600 /var/run/docker.sock", { user: "root" });
    const unavailable = $('[data-testid="containers-unavailable"]');
    await unavailable.waitForDisplayed({ timeout: 20_000 });
    expect(await textOf(unavailable)).toMatch(
      /INDISPONIBLE accès à Docker refusé\. Les autres mesures restent à jour\./i,
    );
    const before = await surveyTime();
    await waitNextSurvey();
    expect(await surveyTime()).not.toBe(before);
    expect(await $('[data-testid="cpu-value"]').isDisplayed()).toBe(true);

    sh("chmod 660 /var/run/docker.sock", { user: "root" });
    await $('[data-testid="container-atlas-limite"]').waitForDisplayed({
      timeout: 20_000,
    });
  });

  it("disque : statvfs refusé sur un point de montage, les autres restent", async function () {
    // Système de fichiers de test sous un dossier fermé au compte de
    // l'application (root seul y entre).
    let mounted = true;
    try {
      sh(
        `mkdir -p ${LOCKED} && chmod 700 /srv/prive && truncate -s 16M /srv/disque.img && mkfs.ext4 -q -F /srv/disque.img && mount -o loop /srv/disque.img ${LOCKED}`,
        { user: "root" },
      );
    } catch {
      mounted = false;
    }
    if (!mounted) {
      // Montage loop refusé par l'hôte Docker : cas laissé au test manuel.
      this.skip();
    }
    const row = $(diskRow(LOCKED));
    await row.waitForDisplayed({ timeout: 90_000 });
    expect(await textOf(row)).toMatch(/accès refusé INDISPONIBLE$/i);
    expect(await $(diskRow("/var/lib/docker")).isDisplayed()).toBe(true);
  });
});
