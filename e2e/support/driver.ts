/**
 * Pilotes WebDriver : msedgedriver de la version exacte du runtime WebView2
 * installé (sinon la session se bloque à la connexion, cf. doc Tauri), et
 * tauri-driver, qui fait le lien entre WebdriverIO et msedgedriver.
 */
import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { CACHE_DIR, NATIVE_DRIVER_PORT, TAURI_DRIVER_PORT } from "./env.ts";

/** Clé d'EdgeUpdate du runtime WebView2 « Evergreen ». */
const WEBVIEW2_CLIENT = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
const WEBVIEW2_KEYS = [
  `HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\${WEBVIEW2_CLIENT}`,
  `HKLM\\SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\${WEBVIEW2_CLIENT}`,
  `HKCU\\SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\${WEBVIEW2_CLIENT}`,
];

export function webview2Version(): string {
  for (const key of WEBVIEW2_KEYS) {
    try {
      const out = execFileSync("reg", ["query", key, "/v", "pv"], {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
      });
      const match = /pv\s+REG_SZ\s+([\d.]+)/.exec(out);
      if (match && match[1] !== "0.0.0.0") return match[1];
    } catch {
      // Clé absente : la suivante.
    }
  }
  throw new Error("runtime WebView2 introuvable dans le registre");
}

/** Télécharge (une fois par version) le msedgedriver qui correspond à WebView2. */
export async function ensureEdgeDriver(): Promise<string> {
  const version = process.env.E2E_WEBVIEW2_VERSION ?? webview2Version();
  const dir = path.join(CACHE_DIR, "msedgedriver", version);
  const exe = path.join(dir, "msedgedriver.exe");
  if (fs.existsSync(exe)) return exe;

  const url = `https://msedgedriver.microsoft.com/${version}/edgedriver_win64.zip`;
  console.log(`[e2e] msedgedriver ${version} : ${url}`);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(
      `téléchargement de msedgedriver ${version} : HTTP ${response.status}`,
    );
  }
  fs.mkdirSync(dir, { recursive: true });
  const zip = path.join(dir, "edgedriver_win64.zip");
  fs.writeFileSync(zip, Buffer.from(await response.arrayBuffer()));
  // bsdtar, livré avec Windows 10 et 11, lit les archives zip.
  execFileSync("tar", ["-xf", zip, "-C", dir]);
  fs.rmSync(zip);
  if (!fs.existsSync(exe)) throw new Error(`msedgedriver.exe absent de ${url}`);
  return exe;
}

export function tauriDriverPath(): string {
  const exe = path.join(
    os.homedir(),
    ".cargo",
    "bin",
    process.platform === "win32" ? "tauri-driver.exe" : "tauri-driver",
  );
  if (!fs.existsSync(exe)) {
    throw new Error(
      "tauri-driver absent : « cargo install tauri-driver --locked »",
    );
  }
  return exe;
}

function portOpen(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = net.connect({ host: "127.0.0.1", port });
    socket.once("connect", () => {
      socket.destroy();
      resolve(true);
    });
    socket.once("error", () => resolve(false));
  });
}

async function waitForPort(port: number, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await portOpen(port)) return;
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`rien n'écoute sur le port ${port} après ${timeoutMs} ms`);
}

export async function startTauriDriver(
  edgeDriver: string,
): Promise<ChildProcess> {
  for (const port of [TAURI_DRIVER_PORT, NATIVE_DRIVER_PORT]) {
    if (await portOpen(port)) {
      throw new Error(
        `port ${port} déjà occupé (un tauri-driver ou msedgedriver resté ouvert ?)`,
      );
    }
  }
  const child = spawn(
    tauriDriverPath(),
    [
      "--port",
      String(TAURI_DRIVER_PORT),
      "--native-port",
      String(NATIVE_DRIVER_PORT),
      "--native-driver",
      edgeDriver,
    ],
    { stdio: ["ignore", "inherit", "inherit"] },
  );
  await waitForPort(TAURI_DRIVER_PORT, 15_000);
  return child;
}

/**
 * Sous Windows, tuer tauri-driver laisse msedgedriver (son enfant) ouvert :
 * l'arbre entier est arrêté.
 */
export function stopTauriDriver(child: ChildProcess | null): void {
  if (!child?.pid || child.exitCode !== null) return;
  if (process.platform === "win32") {
    try {
      execFileSync("taskkill", ["/pid", String(child.pid), "/T", "/F"], {
        stdio: "ignore",
      });
    } catch {
      // Déjà terminé.
    }
  } else {
    child.kill();
  }
}
