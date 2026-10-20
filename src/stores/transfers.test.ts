import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../api/files";
import type { Progress } from "../bindings/Progress";
import { useNotify } from "../composables/useNotify";
import { lastToast } from "../testUtils";
import { useConnectionStore } from "./connection";
import { useTransfersStore, type TransferSpec } from "./transfers";

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

  it("vide la file à une nouvelle liaison", async () => {
    const connection = useConnectionStore();
    const transfers = useTransfersStore();
    transfers.enqueue(upload("a.tar"));
    await flushPromises();
    connection.info = { profileId: "p", connectedAt: 1, latencyMs: null };
    await flushPromises();
    expect(transfers.items).toHaveLength(0);
  });
});
