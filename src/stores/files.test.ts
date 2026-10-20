import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as connectionApi from "../api/connection";
import * as api from "../api/files";
import type { FileEntry } from "../bindings/FileEntry";
import { linked, lossEvent } from "../testUtils";
import { useFilesStore } from "./files";

vi.mock("../api/files");
vi.mock("../api/connection");

function entry(name: string): FileEntry {
  return {
    name,
    path: `/srv/${name}`,
    kind: "file",
    size: 1,
    modified: null,
    permissions: "-rw-r--r--",
    link: null,
    residue: null,
    sshAccess: false,
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.mocked(api.listDir).mockResolvedValue([entry("a.txt")]);
});

describe("explorateur et perte de liaison", () => {
  it("fige la dernière lecture réussie du dossier affiché", async () => {
    const connection = linked();
    const files = useFilesStore();
    await files.open("/srv");
    vi.mocked(api.listDir).mockRejectedValueOnce({
      kind: "Network",
      message: "délai dépassé",
    });
    await files.refresh();
    expect(files.entries).toHaveLength(0);

    connection.linkLost(lossEvent());
    expect(files.entries.map((e) => e.name)).toEqual(["a.txt"]);
    expect(files.error).toBeNull();
    expect(files.frozenAt).not.toBeNull();
  });

  it("n'ouvre aucun dossier pendant le LOS", async () => {
    const connection = linked();
    const files = useFilesStore();
    await files.open("/srv");
    connection.linkLost(lossEvent());
    await files.open("/etc");
    expect(api.listDir).toHaveBeenCalledOnce();
    expect(files.path).toBe("/srv");
  });

  it("ignore une lecture en cours au moment de la perte", async () => {
    const connection = linked();
    const files = useFilesStore();
    await files.open("/srv");
    let answer: (list: FileEntry[]) => void = () => {};
    vi.mocked(api.listDir).mockImplementationOnce(
      () => new Promise((resolve) => (answer = resolve)),
    );
    const reading = files.refresh();
    connection.linkLost(lossEvent());
    answer([]);
    await reading;
    expect(files.entries).toHaveLength(1);
    expect(files.loading).toBe(false);
  });

  it("relit le dossier au rétablissement", async () => {
    const connection = linked();
    vi.mocked(connectionApi.connect).mockResolvedValue({
      profileId: "p1",
      connectedAt: 5,
      latencyMs: null,
    });
    const files = useFilesStore();
    await files.open("/srv");
    connection.linkLost(lossEvent());
    await connection.reacquire();
    await flushPromises();
    expect(api.listDir).toHaveBeenCalledTimes(2);
    expect(files.frozenAt).toBeNull();
  });

  it("vide l'explorateur à la fin de la session", async () => {
    const connection = linked();
    const files = useFilesStore();
    await files.open("/srv");
    await connection.disconnect();
    expect(files.path).toBeNull();
    expect(files.entries).toHaveLength(0);
  });
});
