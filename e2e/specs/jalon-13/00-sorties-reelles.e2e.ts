/**
 * docs/tests-jalon-13.md, section 0 : formats des sorties réelles sur
 * lesquels l'écran Ressources repose, vérifiés dans la cible (comme « en SSH
 * sur le VPS avec le compte de l'application »). La mesure du coût de la
 * répartition est relevée et affichée ; la décision reste manuelle.
 */
import { expect } from "@wdio/globals";
import { seedResources } from "../../support/resources.ts";
import { sh } from "../../support/target.ts";

const ID = /^[0-9a-f]{64}$/;

function seconds(command: string): number {
  const start = Date.now();
  sh(`${command} >/dev/null`, { timeout: 600_000 });
  return (Date.now() - start) / 1000;
}

describe("Jalon 13 · 0. Vérification préalable des sorties réelles", () => {
  before(() => seedResources());

  it("fichiers de /proc lisibles malgré leur taille annoncée de 0", () => {
    expect(sh("stat -c %s /proc/loadavg").trim()).toBe("0");
    expect(sh("cat /proc/loadavg")).toMatch(/^\d+\.\d+ \d+\.\d+ \d+\.\d+ /);
  });

  it("extension statvfs@openssh.com du serveur SFTP", () => {
    // Le serveur SFTP contient le nom de l'extension qu'il annonce.
    expect(
      sh("grep -c statvfs@openssh.com /usr/lib/openssh/sftp-server").trim(),
    ).not.toBe("0");
  });

  it("racine de Docker : un chemin absolu", () => {
    expect(sh("docker info --format '{{.DockerRootDir}}'").trim()).toBe(
      "/var/lib/docker",
    );
  });

  it("docker stats sans flux continu : un objet JSON par conteneur", () => {
    const lines = sh(
      "docker stats --no-stream --no-trunc --format '{{json .}}'",
    )
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line) as Record<string, string>);
    const limited = lines.find((l) => l.Name === "atlas-limite");
    expect(limited).toBeDefined();
    expect(limited?.ID).toMatch(ID);
    expect(limited?.CPUPerc).toMatch(/^\d+\.\d+%$/);
    expect(limited?.MemUsage).toMatch(/^[\d.]+[KMG]?i?B \/ 256MiB$/);
  });

  it("docker system df : quatre catégories, tailles décimales", () => {
    const types = sh("docker system df --format '{{json .}}'")
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line) as { Type: string; Size: string });
    expect(types.map((t) => t.Type)).toEqual([
      "Images",
      "Containers",
      "Local Volumes",
      "Build Cache",
    ]);
    for (const t of types) expect(t.Size).toMatch(/^[\d.]+(B|kB|MB|GB|TB)$/);
  });

  it("docker system df -v : un objet JSON avec les volumes", () => {
    const out = JSON.parse(
      sh("docker system df -v --format '{{json .}}'", { timeout: 600_000 }),
    ) as { Volumes: { Name: string; Size: string }[] };
    const big = out.Volumes.find((v) => v.Name === "atlas-gros");
    expect(big?.Size).toMatch(/^[\d.]+(B|kB|MB|GB)$/);
  });

  it("coût de la répartition comparé à l'analyse des volumes (relevé, décision manuelle)", () => {
    const usage = seconds("docker system df");
    const volumes = seconds("docker system df -v");
    console.log(
      `docker system df : ${usage.toFixed(1)} s ; docker system df -v : ${volumes.toFixed(1)} s ; rapport ${((100 * usage) / volumes).toFixed(0)} %`,
    );
    expect(volumes).toBeGreaterThan(0);
  });
});
