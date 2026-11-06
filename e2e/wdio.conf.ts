/**
 * WebdriverIO + tauri-driver, d'après https://v2.tauri.app/develop/tests/webdriver/.
 *
 * - Lanceur (`onPrepare`, une fois) : binaire de test vérifié, msedgedriver
 *   de la version de WebView2, cible Docker démarrée et remise à zéro.
 * - Chaque fichier de spec (`beforeSession`) : configuration d'Atlas neuve,
 *   tauri-driver lancé ; l'application démarre avec la session WebDriver.
 */
import type { ChildProcess } from "node:child_process";
import { $, browser } from "@wdio/globals";
import {
  assertProfilesAreLocal,
  assertTestBinary,
  writeTestConfig,
} from "./support/appConfig.ts";
import {
  ensureEdgeDriver,
  startTauriDriver,
  stopTauriDriver,
} from "./support/driver.ts";
import { APP_BINARY, TAURI_DRIVER_PORT } from "./support/env.ts";
import {
  ensureSshKey,
  hostFingerprint,
  seedFull,
  startTarget,
  stopTarget,
} from "./support/target.ts";

let tauriDriver: ChildProcess | null = null;

function closeTauriDriver() {
  stopTauriDriver(tauriDriver);
  tauriDriver = null;
}

// `afterSession` ne passe pas si la session n'a pas pu démarrer (doc Tauri).
for (const signal of ["exit", "SIGINT", "SIGTERM", "SIGBREAK"] as const) {
  process.on(signal, closeTauriDriver);
}

export const config: WebdriverIO.Config = {
  runner: "local",
  hostname: "127.0.0.1",
  port: TAURI_DRIVER_PORT,
  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: 1,
  capabilities: [
    {
      maxInstances: 1,
      "tauri:options": { application: APP_BINARY },
    } as WebdriverIO.Capabilities,
  ],
  logLevel: "warn",
  bail: 0,
  waitforTimeout: 15_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 1,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { ui: "bdd", timeout: 240_000 },

  onPrepare: async () => {
    assertTestBinary();
    await ensureEdgeDriver();
    ensureSshKey();
    startTarget();
    seedFull();
  },

  beforeSession: async () => {
    writeTestConfig(hostFingerprint());
    tauriDriver = await startTauriDriver(await ensureEdgeDriver());
  },

  before: () => {
    assertProfilesAreLocal();
  },

  // Une fenêtre GO / NO GO restée ouverte par un test en échec bloquerait
  // tous les clics des suivants : Échap vaut NO GO.
  afterTest: async () => {
    const dialog = await $('[role="alertdialog"]');
    if (await dialog.isDisplayed().catch(() => false)) {
      await browser.keys("Escape");
      await dialog.waitForDisplayed({ reverse: true }).catch(() => undefined);
    }
  },

  afterSession: closeTauriDriver,

  // E2E_KEEP_TARGET=1 garde la cible pour enchaîner les lancements.
  onComplete: () => {
    if (!process.env.E2E_KEEP_TARGET) stopTarget();
  },
};
