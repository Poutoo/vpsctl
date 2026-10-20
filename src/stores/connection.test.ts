import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../api/connection";
import type { AppError } from "../bindings/AppError";
import type { ConnectStep } from "../bindings/ConnectStep";
import { linked, lossEvent } from "../testUtils";
import { useConnectionStore } from "./connection";

vi.mock("../api/connection");

const info = { profileId: "p1", connectedAt: 1, latencyMs: 40 };

beforeEach(() => {
  setActivePinia(createPinia());
  vi.mocked(api.serverInfo).mockResolvedValue({
    os: "Ubuntu",
    docker: "29",
    systemd: "259",
  });
});

describe("connexion", () => {
  it("enregistre les étapes puis passe en connecté", async () => {
    vi.mocked(api.connect).mockImplementation(async (_id, _p, onStep) => {
      onStep({ step: "aos" } as ConnectStep);
      return info;
    });
    const connection = useConnectionStore();
    await connection.connect("p1");
    expect(connection.phase).toBe("connected");
    expect(connection.isConnected).toBe(true);
    expect(connection.steps.map((s) => s.step.step)).toEqual(["aos"]);
    await flushPromises();
    expect(connection.serverInfo?.os).toBe("Ubuntu");
  });

  it.each<[AppError, string]>([
    [{ kind: "UnknownHostKey", fingerprint: "SHA256:x" }, "hostKeyUnknown"],
    [
      { kind: "HostKeyMismatch", expected: "SHA256:a", received: "SHA256:b" },
      "hostKeyMismatch",
    ],
    [{ kind: "PassphraseRequired" }, "passphrase"],
    [{ kind: "PassphraseIncorrect" }, "passphrase"],
    [{ kind: "Cancelled" }, "idle"],
    [{ kind: "AuthFailed" }, "failed"],
  ])("traduit %o en phase %s", async (error, phase) => {
    vi.mocked(api.connect).mockRejectedValue(error);
    const connection = useConnectionStore();
    await connection.connect("p1");
    expect(connection.phase).toBe(phase);
    expect(connection.error).toEqual(error);
  });

  it("garde l'empreinte présentée et l'empreinte attendue", async () => {
    vi.mocked(api.connect).mockRejectedValue({
      kind: "HostKeyMismatch",
      expected: "SHA256:a",
      received: "SHA256:b",
    });
    const connection = useConnectionStore();
    await connection.connect("p1");
    expect(connection.expectedFingerprint).toBe("SHA256:a");
    expect(connection.presentedFingerprint).toBe("SHA256:b");
  });

  it("ignore le résultat d'une tentative interrompue", async () => {
    let finish: (value: typeof info) => void = () => {};
    vi.mocked(api.connect).mockImplementation(
      () => new Promise((resolve) => (finish = resolve)),
    );
    const connection = useConnectionStore();
    const attempt = connection.connect("p1");
    await connection.cancel();
    finish(info);
    await attempt;
    expect(connection.phase).toBe("idle");
    expect(connection.isConnected).toBe(false);
  });

  it("mémorise la passphrase puis se connecte avec le trousseau", async () => {
    vi.mocked(api.connect)
      .mockRejectedValueOnce({ kind: "PassphraseRequired" })
      .mockResolvedValueOnce(info);
    const connection = useConnectionStore();
    await connection.connect("p1");
    await connection.submitPassphrase("secret", true);
    expect(api.setKeyPassphrase).toHaveBeenCalledWith("p1", "secret");
    expect(vi.mocked(api.connect).mock.calls[1][1]).toBeNull();
    expect(connection.phase).toBe("connected");
  });

  it("redemande une passphrase incorrecte sans relancer la connexion", async () => {
    vi.mocked(api.connect).mockRejectedValueOnce({
      kind: "PassphraseRequired",
    });
    vi.mocked(api.setKeyPassphrase).mockRejectedValueOnce({
      kind: "PassphraseIncorrect",
    });
    const connection = useConnectionStore();
    await connection.connect("p1");
    await connection.submitPassphrase("faux", true);
    expect(connection.passphraseRejected).toBe(true);
    expect(api.connect).toHaveBeenCalledOnce();
  });
});

describe("déconnexion", () => {
  it("vide l'état de la liaison", async () => {
    vi.mocked(api.connect).mockResolvedValue(info);
    const connection = useConnectionStore();
    await connection.connect("p1");
    await connection.disconnect();
    expect(api.disconnect).toHaveBeenCalledOnce();
    expect(connection.isConnected).toBe(false);
    expect(connection.phase).toBe("idle");
    expect(connection.serverInfo).toBeNull();
  });
});

describe("perte de liaison", () => {
  function watchMoments(connection: ReturnType<typeof useConnectionStore>) {
    const moments: string[] = [];
    for (const moment of ["lost", "restored", "ended"] as const) {
      connection.on(moment, () => moments.push(moment));
    }
    return moments;
  }

  it("passe en LOS une seule fois, avec la cause de l'événement", () => {
    const connection = linked();
    const moments = watchMoments(connection);
    connection.linkLost(lossEvent({ cause: "reset" }));
    connection.linkLost(lossEvent({ cause: "timeout" }));

    expect(moments).toEqual(["lost"]);
    expect(connection.phase).toBe("lost");
    expect(connection.isConnected).toBe(false);
    expect(connection.hasSession).toBe(true);
    expect(connection.loss).toMatchObject({
      profileId: "p1",
      cause: "reset",
      lastReceivedAt: 1_000,
      attempts: 0,
    });
  });

  it("ignore un événement sans liaison ou d'un autre profil", () => {
    const connection = useConnectionStore();
    connection.linkLost(lossEvent());
    expect(connection.lost).toBe(false);

    linked();
    connection.linkLost(lossEvent({ profileId: "autre" }));
    expect(connection.lost).toBe(false);
  });

  it("ne déclenche jamais de LOS pendant une déconnexion volontaire", async () => {
    const connection = linked();
    const moments = watchMoments(connection);
    vi.mocked(api.disconnect).mockImplementation(async () => {
      connection.linkLost(lossEvent());
    });
    await connection.disconnect();

    expect(connection.lost).toBe(false);
    expect(connection.hasSession).toBe(false);
    expect(moments).toEqual(["ended"]);
  });

  it("constate la perte au sondage puis précise la cause avec l'événement", async () => {
    const connection = linked();
    vi.mocked(api.connectionStatus).mockResolvedValue(null);
    await connection.refreshStatus();
    expect(connection.loss).toMatchObject({
      cause: "unknown",
      provisional: true,
    });

    connection.linkLost(lossEvent({ cause: "closed", detail: "maintenance" }));
    expect(connection.loss).toMatchObject({
      cause: "closed",
      detail: "maintenance",
      provisional: false,
    });
  });

  it("rétablit la liaison au clic, sans reconnexion automatique", async () => {
    vi.useFakeTimers();
    const connection = linked();
    const moments = watchMoments(connection);
    connection.linkLost(lossEvent());
    expect(api.connect).not.toHaveBeenCalled();

    vi.mocked(api.connect).mockResolvedValue({ ...info, connectedAt: 9 });
    await connection.reacquire();
    expect(api.connect).toHaveBeenCalledWith("p1", null, expect.any(Function));
    expect(connection.lost).toBe(false);
    expect(connection.phase).toBe("connected");
    expect(connection.restored).not.toBeNull();
    expect(moments).toEqual(["lost", "restored"]);

    vi.advanceTimersByTime(5_000);
    expect(connection.restored).toBeNull();
    vi.useRealTimers();
  });

  it("reste en LOS après un échec et compte chaque tentative", async () => {
    const connection = linked();
    connection.linkLost(lossEvent());
    vi.mocked(api.connect).mockRejectedValue({
      kind: "Network",
      message: "hôte injoignable",
    });
    await connection.reacquire();
    await connection.reacquire();

    expect(connection.lost).toBe(true);
    expect(connection.phase).toBe("failed");
    expect(connection.loss?.attempts).toBe(2);
  });

  it("demande la passphrase sans quitter le LOS, puis rétablit", async () => {
    const connection = linked();
    connection.linkLost(lossEvent());
    vi.mocked(api.connect)
      .mockRejectedValueOnce({ kind: "PassphraseRequired" })
      .mockResolvedValueOnce(info);
    await connection.reacquire();
    expect(connection.phase).toBe("passphrase");
    expect(connection.lost).toBe(true);

    await connection.submitPassphrase("secret", false);
    expect(connection.lost).toBe(false);
    expect(connection.phase).toBe("connected");
  });

  it("quitte le LOS pour l'alerte si l'empreinte a changé", async () => {
    const connection = linked();
    const moments = watchMoments(connection);
    connection.linkLost(lossEvent());
    vi.mocked(api.connect).mockRejectedValue({
      kind: "HostKeyMismatch",
      expected: "SHA256:a",
      received: "SHA256:b",
    });
    await connection.reacquire();

    expect(connection.phase).toBe("hostKeyMismatch");
    expect(connection.hasSession).toBe(false);
    expect(moments).toEqual(["lost", "ended"]);
  });

  it("revient au LOS si la tentative est annulée", async () => {
    let finish: (value: typeof info) => void = () => {};
    vi.mocked(api.connect).mockImplementation(
      () => new Promise((resolve) => (finish = resolve)),
    );
    const connection = linked();
    connection.linkLost(lossEvent());
    const attempt = connection.reacquire();
    expect(connection.phase).toBe("connecting");

    await connection.cancel();
    finish(info);
    await attempt;
    expect(connection.phase).toBe("lost");
    expect(connection.lost).toBe(true);
  });
});
