/**
 * Build de test : debug, sans installateur, identifiant `dev.poutoo.atlas.e2e`
 * (tauri.e2e.conf.json), dans src-tauri/target/e2e pour ne pas écraser le
 * build de `tauri dev`. Lancé par `npm run e2e:build` (Node exécute le
 * TypeScript directement).
 */
import { spawnSync } from "node:child_process";
import path from "node:path";
import { APP_TARGET_DIR, E2E_DIR, REPO_DIR } from "../support/env.ts";

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
process.exit(result.status ?? 1);
