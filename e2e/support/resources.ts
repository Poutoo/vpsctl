/**
 * Écran Ressources (jalon 13) : navigation, lecture des cartes, saisie des
 * seuils, et ce que le test manuel fait « sur le VPS » (`docker exec` dans
 * la cible).
 */
import fs from "node:fs";
import path from "node:path";
import { $, browser } from "@wdio/globals";
import { OTHER_PROFILE, PROFILE } from "./appConfig.ts";
import { APP_CONFIG_DIR, TARGET } from "./env.ts";
import { sh, shStatus } from "./target.ts";
import { textOf } from "./ui.ts";

type Resource = "disk" | "memory" | "load";
type Level = "attention" | "critical";
type Thresholds = Record<Resource, Record<Level, number>>;

const DEFAULT_THRESHOLDS: Thresholds = {
  disk: { attention: 80, critical: 90 },
  memory: { attention: 85, critical: 95 },
  load: { attention: 1, critical: 2 },
};
/** Valeurs par défaut telles que les champs les affichent, dans leur ordre. */
const DEFAULT_FIELDS = ["80", "90", "85", "95", "1,0", "2,0"];

export const SURVEY = '[data-testid="survey-status"]';
export const DOCKER_MOUNT = "/var/lib/docker";

export function card(name: "cpu" | "memory" | "load" | "network"): string {
  return `[data-testid="card-${name}"]`;
}

export function diskRow(mountPoint: string): string {
  return `[data-testid="disk-${mountPoint}"]`;
}

export function alert(resource: "disk" | "memory" | "load"): string {
  return `[data-testid="alert-${resource}"]`;
}

/** Préparation de la section 1 et état normal (socket, disque, mémoire). */
export function seedResources(): void {
  sh("bash /e2e/seed-resources.sh", { user: "root", timeout: 900_000 });
}

/**
 * Suspend le démon Docker de la cible (SIGSTOP) : une commande `docker`
 * reste alors en attente aussi longtemps que voulu. Sert à observer une
 * analyse en cours et son annulation, l'analyse réelle de la cible ne durant
 * qu'une fraction de seconde.
 */
export function holdDocker(): void {
  sh("pkill -STOP -x dockerd", { user: "root" });
}

export function releaseDocker(): void {
  sh("pkill -CONT -x dockerd || true", { user: "root" });
}

export async function goResources(): Promise<void> {
  await $(
    '//nav[@aria-label="Navigation principale"]//a[contains(., "Ressources")]',
  ).click();
  await $("h1=Ressources du serveur").waitForDisplayed();
}

export async function goFiles(): Promise<void> {
  await $(
    '//nav[@aria-label="Navigation principale"]//a[contains(., "Fichiers")]',
  ).click();
  await $("h1").waitForDisplayed();
}

/** Heure du dernier relevé affichée dans l'en-tête, `null` avant le premier. */
export async function surveyTime(): Promise<string | null> {
  const match = /(\d{2}:\d{2}:\d{2})/.exec(await textOf(SURVEY));
  return match ? match[1] : null;
}

/** Attend un relevé plus récent que celui affiché. */
export async function waitNextSurvey(timeout = 20_000): Promise<void> {
  const before = await surveyTime();
  await browser.waitUntil(
    async () => {
      const now = await surveyTime();
      return now !== null && now !== before;
    },
    { timeout, timeoutMsg: "aucun nouveau relevé" },
  );
}

/** Attend que le CPU, mesuré par différence, ait sa valeur (2e relevé). */
export async function waitCpuValue(timeout = 30_000): Promise<number> {
  const value = $('[data-testid="cpu-value"]');
  await value.waitForDisplayed({ timeout });
  return Number(await value.getText());
}

/** Profil dont les seuils sont affichés, d'après son nom. */
async function shownProfileId(): Promise<string> {
  const name = await textOf('[data-testid="thresholds-profile"]');
  const profile = [PROFILE, OTHER_PROFILE].find((p) => p.name === name);
  if (!profile) throw new Error(`profil des seuils inconnu : « ${name} »`);
  return profile.id;
}

/**
 * Seuils enregistrés d'un profil dans `resource-thresholds.json` : `null`
 * sans entrée (valeurs par défaut), `undefined` si le fichier n'a pas pu
 * être lu à cet instant (remplacement en cours).
 */
function savedThresholds(profileId: string): Thresholds | null | undefined {
  const file = path.join(APP_CONFIG_DIR, "resource-thresholds.json");
  if (!fs.existsSync(file)) return null;
  try {
    const content = JSON.parse(fs.readFileSync(file, "utf8")) as {
      profiles?: Record<string, Thresholds>;
    };
    return content.profiles?.[profileId] ?? null;
  } catch {
    return undefined;
  }
}

/**
 * Saisit un seuil puis attend son sort : enregistré dans le fichier des
 * seuils (300 ms après la frappe), ou refusé avec sa raison affichée.
 */
export async function setThreshold(
  resource: Resource,
  level: Level,
  value: string,
): Promise<void> {
  const input = $(`[data-testid="threshold-${resource}-${level}"]`);
  const error = $(`[data-testid="threshold-error-${resource}-${level}"]`);
  await input.scrollIntoView({ block: "center" });
  const profileId = await shownProfileId();
  await input.setValue(value);
  const expected = Number(value.replace(",", "."));
  await browser.waitUntil(
    async () => {
      const saved = savedThresholds(profileId);
      if (
        saved !== undefined &&
        (saved ?? DEFAULT_THRESHOLDS)[resource][level] === expected
      )
        return true;
      return error.isDisplayed().catch(() => false);
    },
    {
      timeout: 5_000,
      interval: 100,
      timeoutMsg: `seuil ${resource}.${level} à ${value} ni enregistré ni refusé`,
    },
  );
}

export async function thresholdValue(
  resource: Resource,
  level: Level,
): Promise<string> {
  return $(`[data-testid="threshold-${resource}-${level}"]`).getValue();
}

/** « Valeurs par défaut » : entrée du profil retirée, champs remplis. */
export async function resetThresholds(): Promise<void> {
  const profileId = await shownProfileId();
  await $('[data-testid="thresholds-reset"]').click();
  await browser.waitUntil(
    async () => {
      if (savedThresholds(profileId) !== null) return false;
      const fields: string[] = [];
      for (const resource of ["disk", "memory", "load"] as const)
        for (const level of ["attention", "critical"] as const)
          fields.push(await thresholdValue(resource, level));
      return fields.join() === DEFAULT_FIELDS.join();
    },
    {
      timeout: 5_000,
      interval: 100,
      timeoutMsg: "valeurs par défaut jamais rétablies",
    },
  );
}

/** Pourcentage affiché d'un point de montage (« 4 % » → 4). */
export async function displayedDiskPercent(
  mountPoint: string,
): Promise<number> {
  const text = await textOf(
    `${diskRow(mountPoint)} [data-testid="disk-percent"]`,
  );
  return Number(text.replace(/[^\d]/g, ""));
}

/** Occupation calculée comme `df` dans la cible, arrondie comme à l'écran. */
export function targetDiskPercent(mountPoint: string): number {
  const [used, avail] = sh(`df -B1 --output=used,avail ${mountPoint} | tail -1`)
    .trim()
    .split(/\s+/)
    .map(Number);
  return Math.round((100 * used) / (used + avail));
}

/** Mémoire utilisée en %, calculée sur MemAvailable dans la cible. */
export function targetMemoryPercent(): number {
  const info = sh("cat /proc/meminfo");
  const field = (name: string) =>
    Number(new RegExp(`^${name}:\\s+(\\d+)`, "m").exec(info)?.[1] ?? NaN);
  return (
    (100 * (field("MemTotal") - field("MemAvailable"))) / field("MemTotal")
  );
}

/** Charge par cœur sur 5 min dans la cible. */
export function targetLoadPerCore(): number {
  const [load, cores] = sh("cut -d' ' -f2 /proc/loadavg; nproc")
    .trim()
    .split("\n")
    .map(Number);
  return load / cores;
}

/**
 * Processus du compte de l'application dont la ligne correspond. La première
 * lettre passe entre crochets (`[d]ocker`) : le shell qui lance `pgrep` ne
 * se trouve pas lui-même.
 */
export function processes(pattern: string): string[] {
  const regex = `[${pattern[0]}]${pattern.slice(1)}`;
  const { code, out } = shStatus(`pgrep -u ${TARGET.user} -af '${regex}'`);
  return code === 0 ? out.trim().split("\n").filter(Boolean) : [];
}

/**
 * Échantillonne `processes(pattern)` pendant `durationMs` : identifiants des
 * processus vus.
 */
export async function sampleProcesses(
  pattern: string,
  durationMs: number,
): Promise<Set<string>> {
  const seen = new Set<string>();
  const end = Date.now() + durationMs;
  while (Date.now() < end) {
    for (const line of processes(pattern)) seen.add(line.split(" ")[0]);
    await browser.pause(400);
  }
  return seen;
}
