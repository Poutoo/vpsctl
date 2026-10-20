import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../api/files";
import type { Progress } from "../bindings/Progress";
import { useNotify } from "../composables/useNotify";
import * as connectionApi from "../api/connection";
import { lastToast, linked, lossEvent } from "../testUtils";
import { cutByLoss, useTransfersStore, type TransferSpec } from "./transfers";

vi.mock("../api/files");
vi.mock("../api/local");
vi.mock("../api/connection");

const upload = (name: string): TransferSpec => ({
  direction: "upload",
  name,
  localPath: `C:\\envois\\${name}`,
  remotePath: `/srv/${name}`,
  overwrite: false,
});

/** Transferts lancés, dans l'ordre, avec leur rappel de progression. */
let started: { id: string; onProgress: (event: Progress) => void }[];

beforeEach(() => {
  setActivePinia(createPinia());
  useNotify().items.splice(0);
  started = [];
  vi.mocked(api.upload).mockImplementation(async (_l, _r, _o, onProgress) => {
    const id = `t${started.length + 1}`;
    started.push({ id, onProgress });
    return id;
  });
  vi.mocked(api.listDir).mockResolvedValue([]);
});

describe("file d'attente des transferts", () => {
  it("ne garde qu'un transfert actif et enchaîne à la fin", async () => {
    const transfers = useTransfersStore();
    transfers.enqueue(upload("a.tar"));
    transfers.enqueue(upload("b.tar"));
    await flushPromises();
    expect(transfers.items.map((t) => t.status)).toEqual(["active", "queued"]);
    expect(started).toHaveLength(1);

    started[0].onProgress({ type: "progress", transferred: 50, total: 100 });
    expect(transfers.items[0].transferred).toBe(50);
    started[0].onProgress({ type: "done" });
    await flushPromises();

    expect(transfers.items.map((t) => t.status)).toEqual(["done", "active"]);
    expect(transfers.counts).toMatchObject({ done: 1, active: 1, queued: 0 });
    expect(lastToast()).toMatchObject({
      tone: "success",
      title: "Envoyé",
    });
  });

  it("passe en erreur, le signale et lance le suivant", async () => {
    const transfers = useTransfersStore();
    transfers.enqueue(upload("a.tar"));
    transfers.enqueue(upload("b.tar"));
    await flushPromises();
    started[0].onProgress({
      type: "error",
      error: { kind: "Network", message: "coupé" },
    });
    await flushPromises();

    expect(transfers.items[0]).toMatchObject({ status: "error" });
    expect(transfers.items[1].status).toBe("active");
    expect(lastToast()).toMatchObject({
      tone: "error",
      title: "Envoi interrompu",
      message: "coupé",
    });
  });

  it("refuse au lancement : erreur sans identifiant", async () => {
    vi.mocked(api.upload).mockRejectedValueOnce({
      kind: "AlreadyExists",
      path: "/srv/a.tar",
      isDir: false,
    });
    const transfers = useTransfersStore();
    transfers.enqueue(upload("a.tar"));
    await flushPromises();
    expect(transfers.items[0]).toMatchObject({
      status: "error",
      transferId: null,
      error: { kind: "AlreadyExists" },
    });
  });

  it("relance depuis zéro un transfert en erreur", async () => {
    const transfers = useTransfersStore();
    transfers.enqueue(upload("a.tar"));
    await flushPromises();
    started[0].onProgress({ type: "progress", transferred: 40, total: 100 });
    started[0].onProgress({
      type: "error",
      error: { kind: "Network", message: "coupé" },
    });
    transfers.retry(transfers.items[0].key, true);
    await flushPromises();

    expect(transfers.items[0]).toMatchObject({
      status: "active",
      transferred: 0,
      overwrite: true,
    });
    expect(started).toHaveLength(2);
  });

  it("annule dès que l'identifiant est connu", async () => {
    let resolveId: (id: string) => void = () => {};
    vi.mocked(api.upload).mockImplementationOnce(
      () => new Promise((resolve) => (resolveId = resolve)),
    );
    const transfers = useTransfersStore();
    transfers.enqueue(upload("a.tar"));
    await flushPromises();
    void transfers.cancel(transfers.items[0].key);
    expect(api.cancelTransfer).not.toHaveBeenCalled();

    resolveId("t9");
    await flushPromises();
    expect(api.cancelTransfer).toHaveBeenCalledWith("t9");
  });

  it("ne retire pas le transfert actif", async () => {
    const transfers = useTransfersStore();
    transfers.enqueue(upload("a.tar"));
    transfers.enqueue(upload("b.tar"));
    await flushPromises();
    transfers.remove(transfers.items[0].key);
    transfers.remove(transfers.items[1].key);
    expect(transfers.items.map((t) => t.name)).toEqual(["a.tar"]);
  });

  it("ignore les événements d'un transfert qui n'est plus actif", async () => {
    const transfers = useTransfersStore();
    transfers.enqueue(upload("a.tar"));
    await flushPromises();
    started[0].onProgress({ type: "cancelled" });
    started[0].onProgress({ type: "done" });
    expect(transfers.items[0].status).toBe("cancelled");
  });
});

describe("perte de liaison", () => {
  async function twoQueued() {
    const transfers = useTransfersStore();
    transfers.enqueue(upload("assets-v2.tar"));
    transfers.enqueue(upload("dump.sql.gz"));
    await flushPromises();
    started[0].onProgress({ type: "progress", transferred: 41, total: 100 });
    return transfers;
  }

  it("marque le transfert actif en échec et retient la file", async () => {
    const connection = linked();
    const transfers = await twoQueued();
    connection.linkLost(lossEvent());

    expect(transfers.items[0]).toMatchObject({
      status: "error",
      transferred: 41,
      error: { kind: "LinkLost" },
    });
    expect(cutByLoss(transfers.items[0])).toBe(true);
    expect(transfers.items[1]).toMatchObject({ status: "queued", held: true });
    expect(transfers.held).toBe(true);
    expect(started).toHaveLength(1);
    expect(useNotify().items).toHaveLength(0);

    started[0].onProgress({ type: "cancelled" });
    expect(transfers.items[0].status).toBe("error");
  });

  it("ne relance rien pendant le LOS ni au rétablissement", async () => {
    const connection = linked();
    vi.mocked(connectionApi.connect).mockResolvedValue({
      profileId: "p1",
      connectedAt: 5,
      latencyMs: null,
    });
    const transfers = await twoQueued();
    connection.linkLost(lossEvent());
    transfers.retry(transfers.items[0].key);
    transfers.resumeQueue();
    expect(transfers.items[0].status).toBe("error");

    await connection.reacquire();
    await flushPromises();
    expect(started).toHaveLength(1);
    expect(transfers.items[1].status).toBe("queued");
  });

  it("reprend la file sur demande, et « Réessayer » repart de zéro", async () => {
    const connection = linked();
    vi.mocked(connectionApi.connect).mockResolvedValue({
      profileId: "p1",
      connectedAt: 5,
      latencyMs: null,
    });
    const transfers = await twoQueued();
    connection.linkLost(lossEvent());
    await connection.reacquire();

    transfers.resumeQueue();
    await flushPromises();
    expect(transfers.items[1].status).toBe("active");
    expect(transfers.held).toBe(false);

    transfers.retry(transfers.items[0].key);
    expect(transfers.items[0]).toMatchObject({
      status: "queued",
      transferred: 0,
    });
  });

  it("démarre un nouvel envoi après le rétablissement sans libérer la file retenue", async () => {
    const connection = linked();
    vi.mocked(connectionApi.connect).mockResolvedValue({
      profileId: "p1",
      connectedAt: 5,
      latencyMs: null,
    });
    const transfers = await twoQueued();
    connection.linkLost(lossEvent());
    await connection.reacquire();

    transfers.enqueue(upload("nouveau.txt"));
    await flushPromises();
    expect(transfers.items.map((i) => [i.name, i.status])).toEqual([
      ["assets-v2.tar", "error"],
      ["dump.sql.gz", "queued"],
      ["nouveau.txt", "active"],
    ]);
  });

  it("ignore une erreur LinkLost arrivée avant l'événement", async () => {
    linked();
    const transfers = await twoQueued();
    started[0].onProgress({ type: "error", error: { kind: "LinkLost" } });
    await flushPromises();
    expect(useNotify().items).toHaveLength(0);
    expect(started).toHaveLength(1);
    expect(transfers.items[1].status).toBe("queued");
  });

  it("vide la file à la fin de la session", async () => {
    const connection = linked();
    const transfers = await twoQueued();
    await connection.disconnect();
    expect(transfers.items).toHaveLength(0);
  });
});
