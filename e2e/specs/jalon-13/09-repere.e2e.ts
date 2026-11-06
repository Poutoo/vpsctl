/**
 * docs/tests-jalon-13.md, section 9 : repère de navigation. Depuis un autre
 * écran, le relevé de la minute (mémoire et disque) le fait apparaître :
 * la mémoire de la cible est montée par un conteneur qui remplit un tmpfs.
 */
import { $, browser, expect } from "@wdio/globals";
import {
  goFiles,
  goResources,
  resetThresholds,
  seedResources,
  setThreshold,
  targetMemoryPercent,
  waitCpuValue,
} from "../../support/resources.ts";
import { sh } from "../../support/target.ts";
import { connect, goScripts } from "../../support/ui.ts";

const BADGE = '[data-testid^="nav-resources-"]';

async function badge(): Promise<string | null> {
  const element = $(BADGE);
  if (!(await element.isExisting())) return null;
  return element.getAttribute("data-testid");
}

describe("Jalon 13 · 9. Repère de navigation", () => {
  before(async () => {
    seedResources();
    await connect();
    await goResources();
    await waitCpuValue();
  });

  after(() => seedResources());

  it("alerte mémoire active : repère visible depuis Fichiers, critique en cercle", async () => {
    await setThreshold("memory", "attention", "1");
    await browser.waitUntil(async () => (await badge()) !== null);
    expect(await badge()).toBe("nav-resources-attention");
    await goFiles();
    expect(await badge()).toBe("nav-resources-attention");
    expect(
      await $(`${BADGE} [data-testid="shape-triangle"]`).isExisting(),
    ).toBe(true);
    await goResources();
    await setThreshold("memory", "critical", "2");
    await browser.waitUntil(
      async () => (await badge()) === "nav-resources-critical",
    );
    expect(await $(`${BADGE} [data-testid="shape-alert"]`).isExisting()).toBe(
      true,
    );
  });

  it("à 1024 px, le repère reste sur le chiffre IV", async () => {
    await browser.setWindowSize(1024, 680);
    try {
      await goFiles();
      expect(await $(BADGE).isDisplayed()).toBe(true);
    } finally {
      await browser.setWindowSize(1280, 800);
    }
  });

  it("hors de l'écran, le relevé de la minute fait apparaître le repère", async () => {
    await goResources();
    await resetThresholds();
    const percent = Math.round(targetMemoryPercent());
    await setThreshold("memory", "critical", "100");
    await setThreshold("memory", "attention", String(percent + 3));
    await browser.waitUntil(async () => (await badge()) === null);

    await goScripts();
    // Environ 1,5 Go en mémoire partagée, non récupérable.
    sh(
      "docker run -d --name atlas-memoire --shm-size=2g busybox:1.37 sh -c 'dd if=/dev/zero of=/dev/shm/plein bs=1M count=1500 2>/dev/null; sleep 1d'",
    );
    await browser.waitUntil(() => targetMemoryPercent() >= percent + 4, {
      timeout: 30_000,
      timeoutMsg: "mémoire de la cible non montée",
    });
    await browser.waitUntil(async () => (await badge()) !== null, {
      timeout: 75_000,
      timeoutMsg: "repère absent après le relevé de la minute",
    });
  });

  it("la charge ne compte pas hors de l'écran", async () => {
    sh("docker rm -f atlas-memoire");
    await goResources();
    await resetThresholds();
    await setThreshold("load", "attention", "0,1");
    sh("for i in $(seq $(nproc)); do (timeout 200 yes > /dev/null &); done");
    try {
      await $('[data-testid="alert-load"]').waitForDisplayed({
        timeout: 180_000,
      });
      expect(await badge()).not.toBeNull();
      await goScripts();
      await browser.waitUntil(async () => (await badge()) === null, {
        timeoutMsg: "la charge compte encore hors de l'écran",
      });
    } finally {
      sh("pkill -x yes || true");
    }
  });
});
