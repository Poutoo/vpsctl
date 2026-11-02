/**
 * Chemins et réglages communs aux tests E2E. Tout ce qui est propre aux
 * tests vit sous `e2e/.cache/` (ignoré par git) ou dans le dossier de
 * configuration de l'identifiant de test, jamais dans celui d'Atlas.
 */
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const E2E_DIR = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
export const REPO_DIR = path.resolve(E2E_DIR, "..");
export const CACHE_DIR = path.join(E2E_DIR, ".cache");

/** Identifiant de `tauri.e2e.conf.json` : dossiers de configuration séparés. */
export const IDENTIFIER = "dev.poutoo.atlas.e2e";

/** Build de test, dans son propre dossier cargo : `target/debug` reste celui de `tauri dev`. */
export const APP_TARGET_DIR = path.join(REPO_DIR, "src-tauri", "target", "e2e");
export const APP_BINARY = path.join(
  APP_TARGET_DIR,
  "debug",
  process.platform === "win32" ? "atlas-vps.exe" : "atlas-vps",
);

/** `app_config_dir()` de Tauri pour l'identifiant de test. */
export const APP_CONFIG_DIR =
  process.platform === "win32"
    ? path.join(process.env.APPDATA ?? "", IDENTIFIER)
    : path.join(os.homedir(), ".config", IDENTIFIER);

/** Données du moteur web (WebView2) de l'identifiant de test. */
export const WEBVIEW_DATA_DIR =
  process.platform === "win32"
    ? path.join(process.env.LOCALAPPDATA ?? "", IDENTIFIER)
    : path.join(os.homedir(), ".local", "share", IDENTIFIER);

export const SSH_KEY = path.join(CACHE_DIR, "ssh", "id_ed25519");

export const TARGET = {
  container: "atlas-e2e-target",
  composeFile: path.join(E2E_DIR, "target", "compose.yaml"),
  host: "127.0.0.1",
  port: Number(process.env.E2E_SSH_PORT ?? 2222),
  user: "atlas",
  home: "/home/atlas",
  tests: "/home/atlas/atlas-tests",
} as const;

/** Seules destinations admises : la boucle locale, où écoute la cible Docker. */
export const LOOPBACK_HOSTS = new Set(["127.0.0.1", "localhost", "::1"]);

export const TAURI_DRIVER_PORT = 4444;
export const NATIVE_DRIVER_PORT = 4445;
