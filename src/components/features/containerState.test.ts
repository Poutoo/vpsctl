import { describe, expect, it } from "vitest";
import type { Container } from "../../bindings/Container";
import { containerStateDetail, containerStateHelp } from "./containerState";

function container(overrides: Partial<Container>): Container {
  return {
    id: "a".repeat(64),
    name: "worker",
    image: "app:1",
    state: "stopped",
    dockerState: "exited",
    exitCode: 137,
    status: "Exited (137) 9 minutes ago",
    duration: "il y a 9 minutes",
    oomKilled: false,
    memoryLimit: null,
    ...overrides,
  };
}

describe("état d'un conteneur sorti avec le code 137", () => {
  it("explique un manque de mémoire, avec la limite", () => {
    const killed = container({
      state: "failed",
      oomKilled: true,
      memoryLimit: 536_870_912,
    });
    expect(containerStateHelp(killed)).toContain(
      "Tué par manque de mémoire : le noyau a arrêté le processus à la limite de 512 Mo (OOMKilled)",
    );
    expect(containerStateDetail(killed)).toBeNull();
  });

  it("explique un manque de mémoire sans limite propre au conteneur", () => {
    const killed = container({ state: "failed", oomKilled: true });
    expect(containerStateHelp(killed)).toContain(
      "faute de mémoire disponible sur le serveur",
    );
  });

  it("explique un SIGKILL sans manque de mémoire, qui reste arrêté", () => {
    const stopped = container({});
    expect(containerStateHelp(stopped)).toContain(
      "Arrêté par SIGKILL, sans manque de mémoire",
    );
    expect(containerStateDetail(stopped)).toBe("code 137");
  });

  it("donne l'état brut dans les autres cas", () => {
    const running = container({
      state: "nominal",
      dockerState: "running",
      exitCode: null,
      status: "Up 3 hours",
    });
    expect(containerStateHelp(running)).toBe("running · Up 3 hours");
    expect(containerStateDetail(running)).toBeNull();
    expect(
      containerStateDetail(container({ state: "failed", exitCode: 1 })),
    ).toBe("code 1");
  });
});
