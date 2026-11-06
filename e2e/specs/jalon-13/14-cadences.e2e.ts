/**
 * docs/tests-jalon-13.md, section 14 : cadences. `docker stats` ne tourne
 * que quand l'écran Ressources est affiché ; hors de l'écran, aucune
 * commande `docker` n'est lancée par les relevés. La fenêtre réduite reste
 * manuelle : WebDriver ne réduit pas la fenêtre de WebView2.
 */
import { browser, expect } from "@wdio/globals";
import {
  goFiles,
  goResources,
  sampleProcesses,
  seedResources,
  waitCpuValue,
} from "../../support/resources.ts";
import { connect } from "../../support/ui.ts";

describe("Jalon 13 · 14. Cadences", () => {
  before(async () => {
    seedResources();
    await connect();
  });

  it("écran affiché : docker stats par moments, jamais deux à la fois", async () => {
    await goResources();
    await waitCpuValue();
    const seen = await sampleProcesses("docker stats --no-stream", 20_000);
    expect(seen.size).toBeGreaterThan(0);
  });

  it("autre écran : jamais docker stats ni autre commande docker de relevé", async () => {
    await goFiles();
    await browser.pause(6_000);
    expect((await sampleProcesses("docker stats", 20_000)).size).toBe(0);
    expect((await sampleProcesses("docker system df", 5_000)).size).toBe(0);
  });

  it.skip(
    "fenêtre réduite : plus de docker stats — manuel : WebDriver ne réduit pas la fenêtre de WebView2",
  );
});
