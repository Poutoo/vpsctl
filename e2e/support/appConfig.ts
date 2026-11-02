/**
 * Configuration d'Atlas dédiée aux tests, et garde-fous qui empêchent toute
 * liaison vers autre chose que la cible Docker locale.
 */
import fs from "node:fs";
import path from "node:path";
import {
  APP_BINARY,
  APP_CONFIG_DIR,
  IDENTIFIER,
  LOOPBACK_HOSTS,
  SSH_KEY,
  TARGET,
  WEBVIEW_DATA_DIR,
} from "./env.ts";

export const PROFILE = {
  id: "0e2e0000-0000-4000-8000-000000000001",
  name: "atlas-e2e",
} as const;

/** Même serveur sous un autre profil : sa mémoire des projets est séparée. */
export const OTHER_PROFILE = {
  id: "0e2e0000-0000-4000-8000-000000000002",
  name: "atlas-e2e-bis",
} as const;

/**
 * Refuse de continuer si le binaire n'a pas été construit avec
 * `tauri.e2e.conf.json` : il lirait alors les vrais profils, dont le VPS.
 */
export function assertTestBinary(): void {
  if (!fs.existsSync(APP_BINARY)) {
    throw new Error(
      `${APP_BINARY} absent : lancer « npm run test:e2e » (qui construit l'application).`,
    );
  }
  if (!fs.readFileSync(APP_BINARY).includes(Buffer.from(IDENTIFIER))) {
    throw new Error(
      `${APP_BINARY} n'a pas l'identifiant ${IDENTIFIER} : il utiliserait les profils réels. Reconstruire avec « npm run e2e:build ».`,
    );
  }
}

function assertTestConfigDir(): void {
  if (path.basename(APP_CONFIG_DIR) !== IDENTIFIER) {
    throw new Error(`dossier de configuration inattendu : ${APP_CONFIG_DIR}`);
  }
}

/** Vérifie que chaque profil enregistré vise la boucle locale. */
export function assertProfilesAreLocal(): void {
  assertTestConfigDir();
  const file = path.join(APP_CONFIG_DIR, "profiles.json");
  if (!fs.existsSync(file)) return;
  const { profiles } = JSON.parse(fs.readFileSync(file, "utf8")) as {
    profiles: { name: string; host: string }[];
  };
  for (const profile of profiles) {
    if (!LOOPBACK_HOSTS.has(profile.host)) {
      throw new Error(
        `profil « ${profile.name} » vers ${profile.host} : les tests E2E ne visent que la cible locale.`,
      );
    }
  }
}

/**
 * Dossier de configuration neuf : deux profils vers la cible, empreinte déjà
 * acceptée (la vérification de l'empreinte relève du jalon 1), aucun projet
 * mémorisé, aucune donnée du moteur web.
 */
export function writeTestConfig(fingerprint: string): void {
  assertTestConfigDir();
  fs.rmSync(APP_CONFIG_DIR, { recursive: true, force: true });
  fs.rmSync(WEBVIEW_DATA_DIR, { recursive: true, force: true });
  fs.mkdirSync(APP_CONFIG_DIR, { recursive: true });
  const profile = (id: string, name: string, isDefault: boolean) => ({
    id,
    name,
    host: TARGET.host,
    port: TARGET.port,
    user: TARGET.user,
    keyPath: SSH_KEY,
    hostKeyFingerprint: fingerprint,
    isDefault,
  });
  const file = {
    version: 1,
    profiles: [
      profile(PROFILE.id, PROFILE.name, true),
      profile(OTHER_PROFILE.id, OTHER_PROFILE.name, false),
    ],
  };
  fs.writeFileSync(
    path.join(APP_CONFIG_DIR, "profiles.json"),
    JSON.stringify(file, null, 2),
  );
  assertProfilesAreLocal();
}

/** Projets mémorisés par profil, tels qu'Atlas les a écrits. */
export function rememberedProjects(): Record<string, { name: string }[]> {
  const file = path.join(APP_CONFIG_DIR, "compose-projects.json");
  if (!fs.existsSync(file)) return {};
  return (
    JSON.parse(fs.readFileSync(file, "utf8")) as {
      profiles: Record<string, { name: string }[]>;
    }
  ).profiles;
}
