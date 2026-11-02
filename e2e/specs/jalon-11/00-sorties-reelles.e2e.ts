/**
 * docs/tests-jalon-11.md, section 0 : formats des sorties réelles sur
 * lesquels l'application repose, vérifiés dans la cible (comme « en SSH
 * sur le VPS avec le compte de l'application »).
 */
import { expect } from "@wdio/globals";
import { TARGET } from "../../support/env.ts";
import { seed, sh } from "../../support/target.ts";

const LABELS = `docker ps -a --no-trunc --format '{"name":{{json .Names}},"project":{{json (.Label "com.docker.compose.project")}},"configFiles":{{json (.Label "com.docker.compose.project.config_files")}},"envFiles":{{json (.Label "com.docker.compose.project.environment_file")}},"oneoff":{{json (.Label "com.docker.compose.oneoff")}}}'`;

interface Labels {
  name: string;
  project: string;
  configFiles: string;
  envFiles: string;
  oneoff: string;
}

function labels(): Labels[] {
  return sh(LABELS)
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line) as Labels);
}

describe("Jalon 11 · 0. Vérification préalable des sorties réelles", () => {
  before(() => seed());

  it("version du plugin, sans « v »", () => {
    const out = sh('docker compose version --short; echo "code $?"');
    expect(out.trim().split("\n")).toEqual(["5.3.1", "code 0"]);
  });

  it("options globales de sortie en texte simple", () => {
    const out = sh(
      "docker compose --ansi never --progress plain -p nominal restart 2>&1 | cat -A | head",
    );
    expect(out).toMatch(/Container nominal-web-1 +Restarting/);
    expect(out).toMatch(/Container nominal-web-1 +Started/);
    // cat -A montre l'échappement ANSI en ^[ et le retour chariot en ^M.
    expect(out).not.toContain("^[[");
    expect(out).not.toContain("^M");
  });

  it("étiquettes lues dans le même appel que la liste", () => {
    const rows = labels();
    const compose = rows.filter((row) => row.project !== "");

    for (const row of compose) {
      for (const file of row.configFiles.split(",")) {
        expect(file.startsWith(`${TARGET.tests}/${row.project}/`)).toBe(true);
      }
      expect(row.envFiles).toBe(
        row.project === "envfile" ? `${TARGET.tests}/envfile/.env.prod` : "",
      );
      expect(row.oneoff).toBe("False");
    }
    const multi = compose.find((row) => row.project === "multi");
    expect(multi?.configFiles.split(",")).toEqual([
      `${TARGET.tests}/multi/compose.yaml`,
      `${TARGET.tests}/multi/compose.prod.yaml`,
    ]);
    expect(rows.find((row) => row.name === "kuma")).toEqual({
      name: "kuma",
      project: "",
      configFiles: "",
      envFiles: "",
      oneoff: "",
    });
  });

  it("étiquette « oneoff » d'un conteneur de docker compose run", () => {
    sh(
      "cd ~/atlas-tests/nominal && docker compose run -d --name nominal-run-e2e web sleep 600",
    );
    try {
      const row = labels().find((r) => r.name === "nominal-run-e2e");
      expect(row?.oneoff).toBe("True");
    } finally {
      sh("docker rm -f nominal-run-e2e");
    }
  });

  it("étiquette des volumes anonymes (Docker 29)", () => {
    const anonymous = sh(
      "docker volume ls --filter label=com.docker.volume.anonymous --format '{{.Name}}'",
    )
      .trim()
      .split("\n")
      .filter(Boolean);
    expect(anonymous.length).toBeGreaterThan(0);
    const anonMounts = sh(
      "docker inspect anon-db-1 --format '{{range .Mounts}}{{.Name}} {{end}}'",
    );
    for (const name of anonymous) {
      expect(name).toMatch(/^[0-9a-f]{64}$/);
      expect(anonMounts).toContain(name);
    }
    expect(anonymous).not.toContain("anon_named");
    expect(anonymous).not.toContain("nominal_data");
  });

  it("restart et down sans fichier", () => {
    const out = sh(
      'cd /tmp && docker compose -p nominal restart; echo "code $?"',
    );
    expect(out.trim().split("\n").at(-1)).toBe("code 0");
    expect(
      sh(
        "docker inspect -f '{{.State.Running}}' nominal-web-1 nominal-app-1",
      ).trim(),
    ).toBe("true\ntrue");
  });
});
