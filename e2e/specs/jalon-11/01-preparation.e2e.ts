/**
 * docs/tests-jalon-11.md, section 1 : la préparation est faite par
 * e2e/target/seed.sh ; on vérifie qu'elle correspond au tableau.
 */
import { expect } from "@wdio/globals";
import { TARGET } from "../../support/env.ts";
import { seed, sh } from "../../support/target.ts";

function states(): Map<string, string> {
  const out = sh("docker ps -a --format '{{.Names}} {{.State}} {{.Status}}'");
  return new Map(
    out
      .trim()
      .split("\n")
      .map((line) => {
        const [name, ...rest] = line.split(" ");
        return [name, rest.join(" ")] as const;
      }),
  );
}

describe("Jalon 11 · 1. Préparation", () => {
  before(() => seed());

  it("un dossier par projet dans ~/atlas-tests", () => {
    expect(sh(`ls ${TARGET.tests}`).trim().split("\n").sort()).toEqual([
      "anon",
      "envfile",
      "failed",
      "loop",
      "multi",
      "nominal",
      "oom",
      "partial",
      "profiled",
      "slow",
    ]);
  });

  it("projets lancés comme dans le tableau, plus « kuma » hors compose", () => {
    const s = states();
    for (const name of [
      "nominal-web-1",
      "nominal-app-1",
      "partial-web-1",
      "failed-web-1",
      "oom-web-1",
      "loop-web-1",
      "multi-web-1",
      "multi-worker-1",
      "envfile-web-1",
      "anon-db-1",
      "profiled-web-1",
      "profiled-debug-1",
      "kuma",
    ]) {
      expect(`${name} ${s.get(name)}`).toMatch(/ running /);
    }
    expect(s.get("partial-migrate-1")).toMatch(/^exited Exited \(0\)/);
    expect(s.get("failed-job-1")).toMatch(/^exited Exited \(3\)/);
    expect(s.get("oom-hog-1")).toMatch(/^exited Exited \(137\)/);
    expect(s.get("loop-crash-1")).toMatch(/^(running|restarting)/);
    expect(s.get("slow-db-1")).toMatch(/^created/);
    expect(s.get("slow-web-1")).toMatch(/^created/);
  });

  it("hog tué par manque de mémoire à la limite de 32 Mio", () => {
    expect(
      sh(
        "docker inspect -f '{{.State.OOMKilled}} {{.HostConfig.Memory}}' oom-hog-1",
      ).trim(),
    ).toBe(`true ${32 * 1024 * 1024}`);
  });

  it("envfile lancé avec DELAY=1d", () => {
    expect(
      sh("docker inspect -f '{{json .Config.Cmd}}' envfile-web-1").trim(),
    ).toBe('["sleep","1d"]');
  });
});
