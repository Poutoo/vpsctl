/**
 * Build de test : debug, sans installateur, identifiant `dev.poutoo.atlas.e2e`
 * (tauri.e2e.conf.json), dans src-tauri/target/e2e pour ne pas écraser le
 * build de `tauri dev`. Lancé par `npm run e2e:build` (Node exécute le
 * TypeScript directement).
 *
 * Sauté quand le binaire existe et que les sources n'ont pas changé depuis
 * le dernier build réussi (empreinte dans e2e/.cache/build-stamp) ;
 * `E2E_FORCE_BUILD=1` reconstruit quand même.
 */
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import {
  APP_BINARY,
  APP_TARGET_DIR,
  CACHE_DIR,
  E2E_DIR,
  REPO_DIR,
} from "../support/env.ts";

const STAMP = path.join(CACHE_DIR, "build-stamp");

/** Fichiers sans effet sur le binaire ; ceux du build de test restent comptés. */
const OUTSIDE = [
  /^docs\//,
  /^e2e\//,
  /^\.github\//,
  /^\.vscode\//,
  /^[^/]+\.md$/,
];
const BUILD_FILES = ["e2e/tauri.e2e.conf.json", "e2e/scripts/build.ts"];

/**
 * Empreinte des fichiers suivis et des nouveaux fichiers non ignorés : un
 * fichier en cours de modification compte, un fichier supprimé aussi.
 */
function sourcesPrint(): string {
  const files = execFileSync(
    "git",
    ["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
    { cwd: REPO_DIR, encoding: "utf8" },
  )
    .split("\0")
    .filter(
      (file) =>
        file &&
        (BUILD_FILES.includes(file) || !OUTSIDE.some((re) => re.test(file))),
    );
  const hash = createHash("sha256");
  for (const file of [...new Set(files)].sort()) {
    const full = path.join(REPO_DIR, file);
    hash.update(`${file}\0`);
    hash.update(fs.existsSync(full) ? fs.readFileSync(full) : "(supprimé)");
    hash.update("\0");
  }
  return hash.digest("hex");
}

const print = sourcesPrint();
const upToDate =
  !process.env.E2E_FORCE_BUILD &&
  fs.existsSync(APP_BINARY) &&
  fs.existsSync(STAMP) &&
  fs.readFileSync(STAMP, "utf8") === print;

if (upToDate) {
  console.log(
    "[e2e] build à jour : sources inchangées depuis le dernier build (E2E_FORCE_BUILD=1 pour reconstruire).",
  );
  process.exit(0);
}

fs.rmSync(STAMP, { force: true });
const result = spawnSync(
  "npx",
  [
    "tauri",
    "build",
    "--debug",
    "--no-bundle",
    "--config",
    path.join(E2E_DIR, "tauri.e2e.conf.json"),
  ],
  {
    cwd: REPO_DIR,
    stdio: "inherit",
    shell: true,
    env: { ...process.env, CARGO_TARGET_DIR: APP_TARGET_DIR },
  },
);
if (result.status === 0) {
  fs.mkdirSync(CACHE_DIR, { recursive: true });
  fs.writeFileSync(STAMP, print);
}
process.exit(result.status ?? 1);
