/**
 * docs/tests-jalon-13.md, section 8 : hystérésis. Le disque de la cible
 * change peu : le seuil est déplacé autour de son occupation. La mémoire,
 * qui oscille, est observée pendant deux minutes ; la marge de la charge est
 * couverte par les tests Rust (`resources::alerts`), la charge de la cible
 * ne se pilotant pas au centième.
 */
import { $, browser, expect } from "@wdio/globals";
import {
  alert,
  DOCKER_MOUNT,
  displayedDiskPercent,
  goResources,
  seedResources,
  setThreshold,
  waitCpuValue,
} from "../../support/resources.ts";
import { connect, textOf } from "../../support/ui.ts";

async function alertShown(resource: "disk" | "memory"): Promise<boolean> {
  return $(alert(resource))
    .isDisplayed()
    .catch(() => false);
}

describe("Jalon 13 · 8. Hystérésis", () => {
  before(async () => {
    seedResources();
    await connect();
    await goResources();
    await waitCpuValue();
  });

  it("disque : se lève au seuil, reste 2 points sous le seuil, retombe à 4", async () => {
    const percent = await displayedDiskPercent(DOCKER_MOUNT);
    await setThreshold("disk", "critical", "100");
    await setThreshold("disk", "attention", String(percent));
    await $(alert("disk")).waitForDisplayed();

    await setThreshold("disk", "attention", String(percent + 2));
    await browser.pause(6_000);
    expect(await alertShown("disk")).toBe(true);

    await setThreshold("disk", "attention", String(percent + 4));
    await browser.waitUntil(async () => !(await alertShown("disk")), {
      timeoutMsg: "l'alerte disque n'est pas retombée",
    });
  });

  it("disque : même règle pour le seuil critique", async () => {
    const percent = await displayedDiskPercent(DOCKER_MOUNT);
    await setThreshold("disk", "attention", "1");
    await setThreshold("disk", "critical", String(percent));
    await browser.waitUntil(async () =>
      /^CRITIQUE/i.test(await textOf(alert("disk"))),
    );
    await setThreshold("disk", "critical", String(percent + 2));
    await browser.pause(6_000);
    expect(await textOf(alert("disk"))).toMatch(/^CRITIQUE/i);
    await setThreshold("disk", "critical", String(percent + 4));
    await browser.waitUntil(async () =>
      /^ATTENTION/i.test(await textOf(alert("disk"))),
    );
  });

  it("mémoire : pas de clignotement avec un seuil à 1 point sous l'occupation", async () => {
    const text = await textOf('[data-testid="card-memory"]');
    const percent = Number(/· (\d+) %/.exec(text)?.[1]);
    await setThreshold("memory", "critical", "100");
    await setThreshold("memory", "attention", String(Math.max(1, percent - 1)));
    await $(alert("memory")).waitForDisplayed();
    const end = Date.now() + 120_000;
    while (Date.now() < end) {
      expect(await alertShown("memory")).toBe(true);
      await browser.pause(2_000);
    }
  });
});
