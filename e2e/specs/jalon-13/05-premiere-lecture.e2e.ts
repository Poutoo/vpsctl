/** docs/tests-jalon-13.md, section 5 : première lecture. */
import { $, browser, expect } from "@wdio/globals";
import {
  card,
  goFiles,
  goResources,
  seedResources,
  waitCpuValue,
} from "../../support/resources.ts";
import { connect, textOf } from "../../support/ui.ts";

const FIRST = "Première lecture en cours · relevé 1 sur 2";

describe("Jalon 13 · 5. Première lecture", () => {
  before(() => seedResources());

  it("squelette, puis CPU et réseau au deuxième relevé", async () => {
    await connect();
    await goResources();
    // CPU et réseau : mesurés par différence, en attente du deuxième relevé.
    expect(await textOf(`${card("cpu")} [data-testid="chart-message"]`)).toBe(
      FIRST,
    );
    expect(
      await textOf(`${card("network")} [data-testid="chart-message"]`),
    ).toBe(FIRST);
    // Mémoire et disque : dès le premier relevé.
    await $('[data-testid="memory-value"]').waitForDisplayed({
      timeout: 15_000,
    });
    await $('[data-testid^="disk-/"]').waitForDisplayed();
    await waitCpuValue(20_000);
    expect(
      await $(`${card("cpu")} [data-testid="chart-message"]`).isExisting(),
    ).toBe(false);
  });

  it("retour sur l'écran : historique gardé, CPU de nouveau en attente", async () => {
    await goFiles();
    await browser.pause(65_000);
    await goResources();
    // Valeur en attente, mais la courbe garde l'historique (pas de message).
    expect(await $('[data-testid="cpu-value"]').isExisting()).toBe(false);
    expect(
      await $(`${card("cpu")} [data-testid="chart-message"]`).isExisting(),
    ).toBe(false);
    expect(await $(`${card("cpu")} path.line`).getAttribute("d")).toMatch(/^M/);
    await waitCpuValue(20_000);
  });
});
