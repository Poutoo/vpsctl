import { getVersion } from "@tauri-apps/api/app";

/** Version de l'application (celle de `tauri.conf.json`). */
export function appVersion(): Promise<string> {
  return getVersion();
}
