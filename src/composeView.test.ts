import { describe, expect, it } from "vitest";
import type { ComposeProject } from "./bindings/ComposeProject";
import type { Container } from "./bindings/Container";
import {
  groupContainers,
  projectDetail,
  projectDetailHelp,
} from "./composeView";

function container(name: string, project: string | null): Container {
  return {
    id: name.padEnd(64, "0"),
    name,
    image: `${name}:1`,
    state: "nominal",
    dockerState: "running",
    exitCode: null,
    status: "Up 4 days",
    duration: "depuis 4 jours",
    oomKilled: false,
    memoryLimit: null,
    project,
    service: name,
    oneoff: false,
  };
}

function project(
  name: string,
  overrides: Partial<ComposeProject> = {},
): ComposeProject {
  return {
    name,
    files: {
      workingDir: `/srv/${name}`,
      configFiles: [`/srv/${name}/compose.yaml`],
      envFiles: [],
    },
    problem: null,
    state: "nominal",
    running: 1,
    total: 1,
    ...overrides,
  };
}

const containers = [
  container("app-web-1", "app"),
  container("app-db-1", "app"),
  container("kuma", null),
];
const projects = [project("app"), project("old", { state: "down" })];

describe("groupContainers", () => {
  it("place les projets puis les conteneurs hors compose", () => {
    const { groups, shown } = groupContainers(containers, projects, "");
    expect(
      groups.map((g) => (g.kind === "project" ? g.project.name : "*")),
    ).toEqual(["app", "old", "*"]);
    expect(groups[0].containers.map((c) => c.name)).toEqual([
      "app-web-1",
      "app-db-1",
    ]);
    expect(groups[1].containers).toEqual([]);
    expect(shown).toBe(3);
  });

  it("montre tout le groupe quand le projet ou son dossier correspond", () => {
    const { groups } = groupContainers(containers, projects, "srv/app");
    expect(groups).toHaveLength(1);
    expect(groups[0].containers).toHaveLength(2);
  });

  it("ne garde que les conteneurs qui correspondent, sinon", () => {
    const { groups, shown } = groupContainers(containers, projects, "db");
    expect(groups).toHaveLength(1);
    expect(groups[0].containers.map((c) => c.name)).toEqual(["app-db-1"]);
    expect(shown).toBe(1);
  });

  it("range hors compose un conteneur dont le projet n'est pas listé", () => {
    const { groups } = groupContainers([container("x", "inconnu")], [], "");
    expect(groups).toEqual([
      { kind: "loose", containers: [container("x", "inconnu")] },
    ]);
  });
});

describe("détail d'un projet", () => {
  it("donne les noms des fichiers, chemins complets en infobulle", () => {
    const multi = project("app", {
      files: {
        workingDir: "/srv/app",
        configFiles: ["/srv/app/compose.yaml", "/srv/app/compose.prod.yaml"],
        envFiles: ["/srv/app/.env.prod"],
      },
    });
    expect(projectDetail(multi)).toBe("compose.yaml + compose.prod.yaml");
    expect(projectDetailHelp(multi)).toBe(
      "-f /srv/app/compose.yaml\n-f /srv/app/compose.prod.yaml\n--env-file /srv/app/.env.prod",
    );
  });

  it("signale un projet descendu ou des étiquettes inutilisables", () => {
    expect(projectDetail(project("old", { state: "down" }))).toBe(
      "aucun conteneur · mémorisé",
    );
    expect(
      projectDetail(
        project("x", { files: null, problem: { kind: "divergent" } }),
      ),
    ).toBe("étiquettes différentes d'un conteneur à l'autre");
  });
});
