/**
 * Pilotage de la cible Docker (le « faux VPS ») depuis les tests : ce que le
 * test manuel fait « en SSH sur le VPS » passe ici par `docker exec`.
 */
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { SSH_KEY, TARGET } from "./env.ts";

function docker(args: string[], timeout = 120_000): string {
  return execFileSync("docker", args, {
    encoding: "utf8",
    timeout,
    stdio: ["ignore", "pipe", "pipe"],
  });
}

/** Commande bash dans la cible, par défaut avec le compte de l'application. */
export function sh(
  command: string,
  options: { user?: "atlas" | "root"; timeout?: number } = {},
): string {
  return docker(
    [
      "exec",
      "-u",
      options.user ?? "atlas",
      "-w",
      TARGET.home,
      TARGET.container,
      "bash",
      "-c",
      command,
    ],
    options.timeout,
  );
}

/** Comme `sh`, sans lever d'exception : code de sortie et sorties. */
export function shStatus(command: string): { code: number; out: string } {
  try {
    return { code: 0, out: sh(command) };
  } catch (error) {
    const e = error as { status?: number; stdout?: string; stderr?: string };
    return { code: e.status ?? 1, out: `${e.stdout ?? ""}${e.stderr ?? ""}` };
  }
}

/**
 * Remet à l'état de départ les projets dont l'empreinte a changé depuis leur
 * dernier semis, et le conteneur hors compose ; avec des noms, ces projets
 * sans vérification.
 */
export function seed(...projects: string[]): void {
  docker(
    ["exec", TARGET.container, "bash", "/e2e/seed.sh", ...projects],
    300_000,
  );
}

/** Tous les projets, sans vérification (lancement de la suite). */
export function seedFull(): void {
  docker(["exec", TARGET.container, "bash", "/e2e/seed.sh", "--full"], 300_000);
}

/** Clé de test, générée une fois (sans passphrase : jamais de trousseau). */
export function ensureSshKey(): void {
  if (fs.existsSync(SSH_KEY)) return;
  fs.mkdirSync(path.dirname(SSH_KEY), { recursive: true });
  execFileSync("ssh-keygen", [
    "-q",
    "-t",
    "ed25519",
    "-N",
    "",
    "-C",
    "atlas-e2e",
    "-f",
    SSH_KEY,
  ]);
}

export function startTarget(): void {
  docker(
    ["compose", "-f", TARGET.composeFile, "up", "-d", "--build", "--wait"],
    900_000,
  );
}

export function stopTarget(): void {
  docker(["compose", "-f", TARGET.composeFile, "down"], 120_000);
}

/** Empreinte de la clé hôte, lue comme sur la console de l'hébergeur. */
export function hostFingerprint(): string {
  const out = docker([
    "exec",
    TARGET.container,
    "ssh-keygen",
    "-lf",
    "/etc/ssh/ssh_host_ed25519_key.pub",
  ]);
  const match = /SHA256:[A-Za-z0-9+/]{43}/.exec(out);
  if (!match) throw new Error(`empreinte illisible : ${out}`);
  return match[0];
}

/** Coupure silencieuse : la cible gèle, aucun paquet n'est fermé ni refusé. */
export function freezeTarget(): void {
  docker(["pause", TARGET.container]);
}

export function thawTarget(): void {
  docker(["unpause", TARGET.container]);
}

/** Masque le plugin compose (section 3), remis en place par `seed`. */
export function hideComposePlugin(): void {
  sh(
    "p=/usr/libexec/docker/cli-plugins/docker-compose; [ -e $p ] && mv $p $p.off || true",
    { user: "root" },
  );
}

export function restoreComposePlugin(): void {
  sh(
    "p=/usr/libexec/docker/cli-plugins/docker-compose; [ -e $p ] || mv $p.off $p",
    { user: "root" },
  );
}

/**
 * Durée avant que `db` du projet `slow` soit sain (40 s après un semis).
 * Prise en compte au prochain `up` qui recrée `db`.
 */
export function setSlowDelay(seconds: number): void {
  sh(
    `sed -i -E 's/sleep [0-9]+; touch/sleep ${seconds}; touch/' ${TARGET.tests}/slow/compose.yaml`,
  );
}

/** Conteneurs d'un projet, d'après Docker (pas d'après l'interface). */
export function projectContainers(project: string): string[] {
  return sh(
    `docker ps -a --filter label=com.docker.compose.project=${project} --format '{{.Names}}'`,
  )
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .sort();
}

export function volumes(): string[] {
  return sh("docker volume ls --format '{{.Name}}'")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
}
