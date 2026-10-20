import { describe, expect, it, vi } from "vitest";
import type { LogEvent } from "../bindings/LogEvent";
import type { LogLine } from "../bindings/LogLine";
import {
  createLogSession,
  isJournalError,
  type StreamDriver,
} from "./logSession";

function line(text: string, overrides: Partial<LogLine> = {}): LogLine {
  return {
    text,
    timestamp: null,
    origin: "stdout",
    priority: null,
    missing: false,
    ...overrides,
  };
}

/** Pilote simulé : chaque ouverture garde son rappel d'événements. */
function fakeDriver() {
  const opened: ((event: LogEvent) => void)[] = [];
  const closed: string[] = [];
  let failure: unknown = null;
  const driver: StreamDriver = {
    open: vi.fn(async (onEvent) => {
      if (failure) throw failure;
      opened.push(onEvent);
      return `flux-${opened.length}`;
    }),
    close: vi.fn(async (id: string) => {
      closed.push(id);
    }),
  };
  return {
    driver,
    closed,
    emit: (event: LogEvent, index = opened.length - 1) => opened[index](event),
    failWith: (error: unknown) => (failure = error),
  };
}

const lines = (...texts: string[]): LogEvent => ({
  type: "lines",
  lines: texts.map((t) => line(t)),
  dropped: 0,
});

describe("isJournalError", () => {
  it("ne marque en erreur que le journal de priorité 0 à 3", () => {
    expect(isJournalError(line("a", { origin: "journal", priority: 3 }))).toBe(
      true,
    );
    expect(isJournalError(line("a", { origin: "journal", priority: 4 }))).toBe(
      false,
    );
    expect(isJournalError(line("a", { origin: "stderr", priority: 0 }))).toBe(
      false,
    );
  });
});

describe("createLogSession", () => {
  it("passe de starting à live puis à ended avec le code de sortie", async () => {
    const fake = fakeDriver();
    const onFinish = vi.fn();
    const session = createLogSession(fake.driver, { onFinish });

    const opening = session.start();
    expect(session.status).toBe("starting");
    expect(await opening).toBe(true);
    expect(session.status).toBe("live");
    expect(session.active).toBe(true);

    fake.emit({ type: "ended", exitCode: 2 });
    expect(session.status).toBe("ended");
    expect(session.exitCode).toBe(2);
    expect(session.endedAt).not.toBeNull();
    expect(onFinish).toHaveBeenCalledOnce();
  });

  it("passe en erreur sur un événement error", async () => {
    const fake = fakeDriver();
    const session = createLogSession(fake.driver);
    await session.start();
    fake.emit({ type: "error", message: "liaison perdue" });
    expect(session.status).toBe("error");
    expect(session.error).toBe("liaison perdue");
  });

  it("traduit l'échec d'ouverture en message", async () => {
    const fake = fakeDriver();
    fake.failWith({ kind: "InvalidInput", message: "4 flux au maximum" });
    const session = createLogSession(fake.driver);
    expect(await session.start()).toBe(false);
    expect(session.status).toBe("error");
    expect(session.error).toBe("4 flux au maximum");
  });

  it("garde au plus `capacity` lignes et tient les compteurs à jour", async () => {
    const fake = fakeDriver();
    const session = createLogSession(fake.driver, { capacity: 3 });
    await session.start();
    fake.emit({
      type: "lines",
      lines: [line("e1", { origin: "stderr" }), line("o1"), line("o2")],
      dropped: 0,
    });
    expect(session.stderrCount).toBe(1);

    fake.emit({
      type: "lines",
      lines: [line("j1", { origin: "journal", priority: 2 })],
      dropped: 0,
    });
    expect(session.lines.map((l) => l.text)).toEqual(["o1", "o2", "j1"]);
    expect(session.stderrCount).toBe(0);
    expect(session.errorCount).toBe(1);
    expect(session.received).toBe(4);
  });

  it("ajoute un avis local pour les lignes écartées", async () => {
    const fake = fakeDriver();
    const session = createLogSession(fake.driver);
    await session.start();
    fake.emit({ type: "lines", lines: [line("a")], dropped: 12 });
    const last = session.lines[session.lines.length - 1];
    expect(last.notice).toBe(true);
    expect(last.text).toBe("12 lignes non affichées : débit trop élevé");
  });

  it("arrête le flux sans appeler onFinish et ignore ses événements tardifs", async () => {
    const fake = fakeDriver();
    const onFinish = vi.fn();
    const session = createLogSession(fake.driver, { onFinish });
    await session.start();
    await session.stop();
    expect(session.status).toBe("stopped");
    expect(fake.closed).toEqual(["flux-1"]);

    fake.emit(lines("tardive"));
    fake.emit({ type: "ended", exitCode: 0 });
    expect(session.lines).toHaveLength(0);
    expect(session.status).toBe("stopped");
    expect(onFinish).not.toHaveBeenCalled();
  });

  it("referme un flux arrêté pendant son ouverture", async () => {
    const fake = fakeDriver();
    const session = createLogSession(fake.driver);
    const opening = session.start();
    await session.stop();
    expect(await opening).toBe(false);
    expect(fake.closed).toEqual(["flux-1"]);
  });

  it("rejoue sur la même source en vidant la mémoire", async () => {
    const fake = fakeDriver();
    const session = createLogSession(fake.driver);
    await session.start();
    fake.emit(lines("ancienne"));
    await session.replay();
    expect(fake.closed).toEqual(["flux-1"]);
    expect(session.lines).toHaveLength(0);
    expect(session.status).toBe("live");

    fake.emit(lines("ignorée"), 0);
    fake.emit(lines("nouvelle"), 1);
    expect(session.lines.map((l) => l.text)).toEqual(["nouvelle"]);
  });
});

describe("interruption par la perte de liaison", () => {
  it("passe en lost sans fermer le flux ni appeler onFinish", async () => {
    const fake = fakeDriver();
    const onFinish = vi.fn();
    const session = createLogSession(fake.driver, { onFinish });
    await session.start();
    session.interrupt();
    expect(session.status).toBe("lost");
    expect(session.active).toBe(false);
    expect(session.endedAt).not.toBeNull();
    expect(fake.closed).toEqual([]);
    expect(onFinish).not.toHaveBeenCalled();

    fake.emit({ type: "error", message: "tardive" });
    expect(session.status).toBe("lost");
  });

  it("ne touche pas un flux déjà terminé", async () => {
    const fake = fakeDriver();
    const session = createLogSession(fake.driver);
    await session.start();
    fake.emit({ type: "ended", exitCode: 0 });
    session.interrupt();
    expect(session.status).toBe("ended");
  });

  it("reprend par `resume` du pilote en gardant les lignes", async () => {
    const fake = fakeDriver();
    const resume = vi.fn(async () => "repris");
    const session = createLogSession({ ...fake.driver, resume });
    await session.start();
    fake.emit(lines("avant"));
    session.interrupt();

    expect(await session.resume()).toBe(true);
    expect(resume).toHaveBeenCalledOnce();
    expect(session.status).toBe("live");
    expect(session.lines.map((l) => l.notice)).toEqual([false, true]);
  });
});
