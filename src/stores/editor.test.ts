import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../api/editor";
import type { TextDocument } from "../bindings/TextDocument";
import { useConfirm } from "../composables/useConfirm";
import { useEditorActions } from "../composables/useEditorActions";
import { useNotify } from "../composables/useNotify";
import { lastToast, linked, lossEvent } from "../testUtils";
import { useEditorStore } from "./editor";

vi.mock("../api/editor");
vi.mock("../api/connection");

function document(text: string, overrides: Partial<TextDocument> = {}) {
  return {
    kind: "text" as const,
    file: {
      path: "/home/deploy/app/.env",
      name: ".env",
      size: text.length,
      modified: 1_000,
      permissions: "-rw-------",
      mode: "600",
      owner: { uid: 1000, gid: 1000, user: "deploy", group: "deploy" },
      dir: "/home/deploy/app",
      dirPermissions: "drwxr-xr-x",
      dirOwner: { uid: 1000, gid: 1000, user: "deploy", group: "deploy" },
      account: "deploy",
    },
    text,
    bom: false,
    lineEnding: "lf" as const,
    snapshot: { sha256: "open", size: text.length, modified: 1_000 },
    readOnly: [],
    sshFile: null,
    openedAt: 5_000,
    ...overrides,
  };
}

const snapshot = (sha256: string) => ({ sha256, size: 1, modified: 2_000 });

/** Répond à la prochaine fenêtre GO / NO GO. */
async function answer(choice: "go" | "nogo" | "alternative" | "link") {
  await flushPromises();
  const { state, settle } = useConfirm();
  expect(state.open).toBe(true);
  settle(choice);
  await flushPromises();
}

beforeEach(() => {
  setActivePinia(createPinia());
  useNotify().items.splice(0);
  vi.mocked(api.fileOpen).mockResolvedValue(document("A=1\n"));
});

describe("éditeur", () => {
  it("n'a rien à enregistrer tant que le texte est celui de l'ouverture", async () => {
    linked();
    const editor = useEditorStore();
    await editor.open("/home/deploy/app/.env");
    expect(editor.dirty).toBe(false);
    expect(editor.canSave).toBe(false);
    editor.setText("A=2\n");
    expect(editor.canSave).toBe(true);
    editor.setText("A=1\n");
    expect(editor.dirty).toBe(false);
  });

  it("enregistre avec l'instantané de l'ouverture puis celui de l'enregistrement", async () => {
    linked();
    const editor = useEditorStore();
    const actions = useEditorActions();
    await editor.open("/home/deploy/app/.env");
    vi.mocked(api.fileSave).mockResolvedValue({
      outcome: "saved",
      snapshot: snapshot("after"),
      verified: true,
      savedAt: 9_000,
    });
    editor.setText("A=2\n");
    await actions.save();
    expect(api.fileSave).toHaveBeenCalledWith({
      path: "/home/deploy/app/.env",
      text: "A=2\n",
      expected: { sha256: "open", size: 4, modified: 1_000 },
      bom: false,
      lineEnding: "lf",
      mode: 0o600,
    });
    expect(editor.dirty).toBe(false);
    expect(editor.saved).toEqual({ at: 9_000, verified: true });
    expect(lastToast().title).toBe("Enregistré");

    editor.setText("A=3\n");
    expect(editor.saved).toBeNull();
    await actions.save();
    expect(vi.mocked(api.fileSave).mock.calls[1][0].expected?.sha256).toBe(
      "after",
    );
  });

  it("conflit : Annuler ne fait rien, Écraser remplace la version vue", async () => {
    linked();
    const editor = useEditorStore();
    const actions = useEditorActions();
    await editor.open("/home/deploy/app/.env");
    editor.setText("A=2\n");
    vi.mocked(api.fileSave).mockResolvedValueOnce({
      outcome: "conflict",
      server: {
        snapshot: snapshot("server"),
        text: "A=1\nB=1\n",
        refusal: null,
      },
    });
    const saving = actions.save();
    await answer("nogo");
    await saving;
    expect(api.fileSave).toHaveBeenCalledOnce();
    expect(editor.text).toBe("A=2\n");

    vi.mocked(api.fileSave)
      .mockResolvedValueOnce({
        outcome: "conflict",
        server: {
          snapshot: snapshot("server"),
          text: "A=1\nB=1\n",
          refusal: null,
        },
      })
      .mockResolvedValueOnce({
        outcome: "saved",
        snapshot: snapshot("mine"),
        verified: true,
        savedAt: 9_000,
      });
    const overwriting = actions.save();
    await answer("go");
    await overwriting;
    expect(vi.mocked(api.fileSave).mock.calls[2][0].expected?.sha256).toBe(
      "server",
    );
    expect(editor.dirty).toBe(false);
  });

  it("conflit : le lien ouvre la différence, Enregistrer… écrase", async () => {
    linked();
    const editor = useEditorStore();
    const actions = useEditorActions();
    await editor.open("/home/deploy/app/.env");
    editor.setText("A=2\n");
    vi.mocked(api.fileSave).mockResolvedValueOnce({
      outcome: "conflict",
      server: {
        snapshot: snapshot("server"),
        text: "A=1\nB=1\n",
        refusal: null,
      },
    });
    const viewing = actions.save();
    await answer("link");
    await viewing;
    expect(editor.view).toBe("diff");

    // Vous reprenez la ligne du serveur : écraser ne perd plus rien.
    editor.setText("A=2\nB=1\n");
    vi.mocked(api.fileSave).mockResolvedValueOnce({
      outcome: "saved",
      snapshot: snapshot("mine"),
      verified: true,
      savedAt: 9_000,
    });
    const saving = actions.saveFromDiff();
    await flushPromises();
    const { state } = useConfirm();
    expect(state.items?.[0].label).toBe(
      "Tous les changements du serveur sont repris",
    );
    await answer("go");
    await saving;
    expect(vi.mocked(api.fileSave).mock.calls[1][0]).toMatchObject({
      text: "A=2\nB=1\n",
      expected: { sha256: "server" },
    });
    expect(editor.view).toBe("edit");
  });

  it("conflit : Recharger relit le fichier", async () => {
    linked();
    const editor = useEditorStore();
    const actions = useEditorActions();
    await editor.open("/home/deploy/app/.env");
    editor.setText("A=2\n");
    vi.mocked(api.fileSave).mockResolvedValue({
      outcome: "conflict",
      server: { snapshot: snapshot("server"), text: "B=1\n", refusal: null },
    });
    vi.mocked(api.fileOpen).mockResolvedValueOnce(document("B=1\n"));
    const reloading = actions.save();
    await answer("alternative");
    await reloading;
    expect(editor.text).toBe("B=1\n");
    expect(editor.dirty).toBe(false);
  });

  it("conflit avec un fichier supprimé : Recréer sans instantané", async () => {
    linked();
    const editor = useEditorStore();
    const actions = useEditorActions();
    await editor.open("/home/deploy/app/.env");
    editor.setText("A=2\n");
    vi.mocked(api.fileSave)
      .mockResolvedValueOnce({
        outcome: "conflict",
        server: { snapshot: null, text: null, refusal: null },
      })
      .mockResolvedValueOnce({
        outcome: "saved",
        snapshot: snapshot("new"),
        verified: true,
        savedAt: 9_000,
      });
    const saving = actions.save();
    await answer("go");
    await saving;
    expect(vi.mocked(api.fileSave).mock.calls[1][0].expected).toBeNull();
    expect(editor.dirty).toBe(false);
  });

  it("pendant un LOS : texte gardé, enregistrement indisponible", async () => {
    const connection = linked();
    const editor = useEditorStore();
    await editor.open("/home/deploy/app/.env");
    editor.setText("A=2\n");
    connection.linkLost(lossEvent());
    expect(editor.canSave).toBe(false);
    expect(editor.text).toBe("A=2\n");
  });

  it("un échec par perte de liaison n'est pas notifié", async () => {
    linked();
    const editor = useEditorStore();
    const actions = useEditorActions();
    await editor.open("/home/deploy/app/.env");
    editor.setText("A=2\n");
    vi.mocked(api.fileSave).mockRejectedValueOnce({ kind: "LinkLost" });
    await actions.save();
    expect(lastToast()).toBeUndefined();
    expect(editor.dirty).toBe(true);
  });

  it("lecture seule : jamais d'enregistrement", async () => {
    linked();
    vi.mocked(api.fileOpen).mockResolvedValueOnce(
      document("A=1\n", { readOnly: ["owner"] }),
    );
    const editor = useEditorStore();
    await editor.open("/etc/caddy/Caddyfile");
    editor.setText("A=2\n");
    expect(editor.canSave).toBe(false);
  });

  it("sortie : sans modification, aucune fenêtre ; sinon GO / NO GO", async () => {
    linked();
    const editor = useEditorStore();
    const actions = useEditorActions();
    await editor.open("/home/deploy/app/.env");
    await expect(actions.confirmExit("close")).resolves.toBe(true);

    editor.setText("A=2\nB=3\n");
    const staying = actions.confirmExit("close");
    await flushPromises();
    const { state } = useConfirm();
    expect(state.title).toBe("Quitter sans enregistrer ?");
    expect(state.items?.[0].label).toBe(
      "+2 −1 lignes modifiées seront perdues",
    );
    expect(state.items?.[0].detail).toBe("A · B");
    await answer("nogo");
    await expect(staying).resolves.toBe(false);
    expect(editor.dirty).toBe(true);

    const leaving = actions.confirmExit("disconnect");
    await answer("go");
    await expect(leaving).resolves.toBe(true);
    expect(editor.dirty).toBe(false);
  });

  it("authorized_keys : GO / NO GO avant chaque enregistrement", async () => {
    linked();
    vi.mocked(api.fileOpen).mockResolvedValueOnce(
      document("ssh-ed25519 AAAA a\n", { sshFile: "authorizedKeys" }),
    );
    vi.mocked(api.authorizedKeysCheck).mockResolvedValue({
      linkKey: { algorithm: "ED25519", comment: "a", tail: "…AAAA" },
      before: { line: 1, options: [] },
      after: null,
      keys: 0,
      certificates: 0,
      authorities: 0,
      unrecognized: [],
      added: [],
      removed: ["a"],
      mode: "600",
      tooOpen: false,
    });
    const editor = useEditorStore();
    const actions = useEditorActions();
    await editor.open("/home/deploy/.ssh/authorized_keys");
    editor.setText("");
    const refused = actions.save();
    await flushPromises();
    const { state } = useConfirm();
    expect(state.title).toBe("Enregistrer sans la clé de cette liaison ?");
    expect(api.authorizedKeysCheck).toHaveBeenCalledWith(
      "/home/deploy/app/.env", // chemin du document simulé
      "ssh-ed25519 AAAA a\n",
      "",
    );
    await answer("nogo");
    await refused;
    expect(api.fileSave).not.toHaveBeenCalled();

    vi.mocked(api.fileSave).mockResolvedValueOnce({
      outcome: "saved",
      snapshot: snapshot("after"),
      verified: true,
      savedAt: 9_000,
    });
    const accepted = actions.save();
    await answer("go");
    await accepted;
    expect(api.fileSave).toHaveBeenCalledOnce();
  });

  it("se vide à la fin de la session", async () => {
    const connection = linked();
    const editor = useEditorStore();
    await editor.open("/home/deploy/app/.env");
    vi.mocked(
      (await import("../api/connection")).disconnect,
    ).mockResolvedValue();
    await connection.disconnect();
    expect(editor.document).toBeNull();
  });
});
