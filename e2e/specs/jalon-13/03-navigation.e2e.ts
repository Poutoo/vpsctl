/** docs/tests-jalon-13.md, section 3 : navigation. */
import { $, $$, browser, expect } from "@wdio/globals";
import { goResources } from "../../support/resources.ts";
import { connect, disconnect, textOf } from "../../support/ui.ts";

const NAV = 'nav[aria-label="Navigation principale"]';

async function navigate(url: string): Promise<void> {
  await browser.execute((to: string) => {
    window.history.pushState({}, "", to);
    window.dispatchEvent(new PopStateEvent("popstate"));
  }, url);
}

describe("Jalon 13 · 3. Navigation", () => {
  it("cinq entrées : IV Ressources, V Réglages ; surtitres numérotés", async () => {
    await connect();
    const items = await $$(`${NAV} .sections a`).map(async (a) =>
      (await a.getText()).replace(/\s+/g, " ").trim(),
    );
    expect(items).toEqual([
      "I Tableau de bord",
      "II Fichiers",
      "III Scripts",
      "IV Ressources",
      "V Réglages",
    ]);
    await goResources();
    expect(await textOf('[data-testid="page-surtitle"]')).toMatch(
      /^IV · Ressources$/i,
    );
    await $(
      `//nav[@aria-label="Navigation principale"]//a[contains(., "Réglages")]`,
    ).click();
    await $("h1=Réglages").waitForDisplayed();
    expect(await textOf('[data-testid="page-surtitle"]')).toMatch(/^V · /);
  });

  it("Ressources demande une liaison, Réglages non", async () => {
    // Couper la liaison depuis un écran qui la demande : retour à la connexion.
    await goResources();
    await disconnect();
    await navigate("/resources");
    await $("h1=Liaison au serveur").waitForDisplayed();
    await navigate("/settings");
    await $("h1=Réglages").waitForDisplayed();
  });
});
