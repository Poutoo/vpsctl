import { describe, expect, it } from "vitest";
import type { FileFacts } from "./bindings/FileFacts";
import {
  ownerLabel,
  readOnlyFacts,
  readOnlyText,
  refusalTag,
} from "./editorFacts";

const file: FileFacts = {
  path: "/srv/app/settings.env",
  name: "settings.env",
  size: 44,
  modified: 1,
  permissions: "-rw-r--r--",
  mode: "644",
  owner: { uid: 0, gid: 0, user: "root", group: "root" },
  dir: "/srv/app",
  dirPermissions: "drwxr-xr-x",
  dirOwner: { uid: 0, gid: 0, user: "root", group: "root" },
  account: "deploy",
};

describe("faits de l'éditeur", () => {
  it("nomme le propriétaire, ou donne ses identifiants", () => {
    expect(ownerLabel(file.owner)).toBe("root:root");
    expect(ownerLabel({ uid: 1001, gid: 1001, user: null, group: null })).toBe(
      "1001:1001",
    );
  });

  it("explique chaque raison de la lecture seule", () => {
    expect(readOnlyText("owner", file).title).toContain(
      "Ce fichier appartient à root ; vous êtes connecté en deploy.",
    );
    expect(readOnlyText("directory", file).short).toBe(
      "dossier non inscriptible",
    );
    expect(readOnlyText("mixedLineEndings", file).advice).toContain(
      "uniformisez",
    );
    expect(readOnlyText("permissions", file).title).toContain(
      "L'utilisateur deploy n'a pas le droit d'écrire",
    );
  });

  it("résume droits, propriétaire et dossier", () => {
    expect(readOnlyFacts(file, ["owner", "directory"])).toBe(
      "Droits -rw-r--r-- 644 · propriétaire root:root · connecté en deploy · dossier /srv/app non inscriptible (drwxr-xr-x root:root)",
    );
    expect(readOnlyFacts(file, ["owner"])).toContain(
      "dossier /srv/app inscriptible",
    );
  });

  it("donne une étiquette à chaque refus", () => {
    expect(refusalTag({ reason: "binary", offset: 16 })).toBe("Binaire");
    expect(refusalTag({ reason: "symlink", target: null })).toBe(
      "Lien symbolique",
    );
  });
});
