import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../api/connection";
import type { AppError } from "../bindings/AppError";
import type { ConnectStep } from "../bindings/ConnectStep";
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
