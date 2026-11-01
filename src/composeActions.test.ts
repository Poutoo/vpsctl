import { describe, expect, it } from "vitest";
import type { ComposeProject } from "./bindings/ComposeProject";
import type { Container } from "./bindings/Container";
import type { DownInfo } from "./bindings/DownInfo";
import {
  containerProgress,
  DOWN_WARNING,
  downConfirmation,
  interruptConsequences,
  projectActionReasons,
} from "./composeActions";

function project(overrides: Partial<ComposeProject> = {}): ComposeProject {
  return {
    name: "urbanflow-mobility",
    files: {
      workingDir: "/srv/urbanflow-mobility",
      configFiles: ["/srv/urbanflow-mobility/docker-compose.yml"],
      envFiles: [],
    },
    problem: null,
    state: "partial",
    running: 2,
    total: 4,
    ...overrides,
  };
}

const idle = { lost: false, busy: false };

describe("disponibilité des actions de projet", () => {
  it("propose tout quand les fichiers sont en place", () => {
    expect(projectActionReasons(project(), { kind: "ok" }, idle)).toEqual({
      up: null,
      pull: null,
      restart: null,
      down: null,
    });
  });

  it("garde Redémarrer et Arrêter quand le dossier manque", () => {
    expect(
      projectActionReasons(project(), { kind: "dirMissing" }, idle),
    ).toEqual({
      up: "dossier introuvable",
      pull: "dossier introuvable",
      restart: null,
      down: null,
    });
    expect(
      projectActionReasons(
        project(),
        { kind: "unreadable", path: "/srv/urbanflow-mobility/.env" },
        idle,
      ).up,
    ).toBe("fichier illisible");
  });

  it("refuse Démarrer et Récupérer sur des étiquettes inutilisables", () => {
    const reasons = projectActionReasons(
      project({ files: null, problem: { kind: "divergent" } }),
      null,
      idle,
    );
    expect(reasons.up).toBe("étiquettes différentes d'un conteneur à l'autre");
    expect(reasons.restart).toBeNull();
    const invalid = projectActionReasons(
      project({ files: null, problem: { kind: "invalidName" } }),
      null,
      idle,
    );
    expect(invalid.down).toBe("nom de projet hors du format de compose");
  });

  it("n'offre que Démarrer et Récupérer à un projet descendu", () => {
    const reasons = projectActionReasons(
      project({ state: "down", running: 0 }),
      { kind: "ok" },
      idle,
    );
    expect(reasons).toEqual({
      up: null,
      pull: null,
      restart: "aucun conteneur",
      down: "aucun conteneur",
    });
  });

  it("bloque tout pendant une commande ou un LOS", () => {
    expect(
      projectActionReasons(project(), null, { lost: false, busy: true }).up,
    ).toBe("commande compose en cours sur ce projet");
    expect(
      projectActionReasons(project(), null, { lost: true, busy: false }).down,
    ).toBe("liaison perdue");
  });
});

describe("conteneurs pendant une commande", () => {
  const running = { dockerState: "running" } as Container;
  const stopped = { dockerState: "exited" } as Container;

  it("ne montre en démarrage que les conteneurs arrêtés", () => {
    expect(containerProgress("up", running)).toBeNull();
    expect(containerProgress("up", stopped)).toBe("Démarrage");
    expect(containerProgress("restart", running)).toBe("Redémarrage");
    expect(containerProgress("down", running)).toBe("Arrêt");
    expect(containerProgress("pull", stopped)).toBeNull();
  });

  it("prévient qu'un up interrompu peut laisser le projet à moitié démarré", () => {
    expect(interruptConsequences("up").join(" ")).toContain("à moitié démarré");
  });
});

function info(overrides: Partial<DownInfo> = {}): DownInfo {
  return {
    project: "urbanflow-mobility",
    command:
      "docker compose --ansi never --progress plain -p 'urbanflow-mobility' down",
    containers: [
      {
        name: "urbanflow-mobility-backend-1",
        service: "backend",
        running: true,
      },
      { name: "urbanflow-mobility-db-1", service: "db", running: true },
      {
        name: "urbanflow-mobility-migrate-1",
        service: "migrate",
        running: false,
      },
      {
        name: "urbanflow-mobility-worker-1",
        service: "worker",
        running: false,
      },
    ],
    networks: ["urbanflow-mobility_default"],
    namedVolumes: ["urbanflow-mobility_pgdata"],
    anonymousVolumes: [],
    binds: [],
    files: {
      workingDir: "/srv/urbanflow-mobility",
      configFiles: ["/srv/urbanflow-mobility/docker-compose.yml"],
      envFiles: [],
    },
    filesStatus: { kind: "ok" },
    ...overrides,
  };
}

describe("GO / NO GO de « Arrêter et supprimer »", () => {
  it("liste ce qui est supprimé, conservé, et le message exact", () => {
    const options = downConfirmation(info());
    expect(options.subject).toBe(
      "urbanflow-mobility · /srv/urbanflow-mobility",
    );
    expect(options.aside).toBe("Données hors volumes perdues");
    expect(options.warning).toBe(DOWN_WARNING);
    expect(options.danger).toBe(true);
    expect(options.removals).toEqual([
      {
        label: "4 conteneurs arrêtés puis supprimés · 2 en marche",
        detail: "backend · db · migrate · worker",
      },
      {
        label: "Réseau du projet supprimé",
        detail: "urbanflow-mobility_default",
      },
    ]);
    expect(options.checks).toEqual([
      {
        label: "Volumes nommés conservés · down sans --volumes",
        detail: "urbanflow-mobility_pgdata",
      },
      { label: "Images conservées · down sans --rmi" },
    ]);
    expect(options.consequences).toEqual([
      "Relance possible avec Démarrer, le fichier compose est présent : docker-compose.yml.",
    ]);
  });

  it("liste les volumes anonymes et les dossiers montés", () => {
    const anon =
      "9b1d4c3e2f1a0b9c8d7e6f5a4b3c2d1e0f9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c";
    const options = downConfirmation(
      info({
        anonymousVolumes: [anon],
        binds: ["/srv/urbanflow-mobility/uploads"],
      }),
    );
    expect(options.removals?.[2]).toEqual({
      label:
        "1 volume anonyme détaché : conservé sur le serveur, mais plus rattaché au projet après le down",
      detail: "9b1d4c3e2f1a",
    });
    expect(options.checks?.[1]).toEqual({
      label: "Dossiers du serveur montés conservés",
      detail: "/srv/urbanflow-mobility/uploads",
    });
  });

  it("dit quand la relance est impossible", () => {
    const options = downConfirmation(
      info({ filesStatus: { kind: "dirMissing" } }),
    );
    expect(options.consequences).toEqual([
      "Relance impossible depuis Atlas : /srv/urbanflow-mobility n'existe plus sur le serveur.",
    ]);
  });
});
