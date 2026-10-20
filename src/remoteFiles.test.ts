import { describe, expect, it } from "vitest";
import type { FileEntry } from "./bindings/FileEntry";
import {
  isDirLike,
  isFileLike,
  localBaseName,
  pathFromSegments,
  pathSegments,
  sortEntries,
} from "./remoteFiles";

function entry(name: string, overrides: Partial<FileEntry> = {}): FileEntry {
  return {
    name,
    path: `/srv/${name}`,
    kind: "file",
    size: 0,
    modified: null,
    permissions: "-rw-r--r--",
    link: null,
    residue: null,
    sshAccess: false,
    ...overrides,
  };
}

const dirLink = entry("lien-dossier", {
  kind: "symlink",
  link: { target: "/opt", targetKind: "dir" },
});
const fileLink = entry("lien-fichier", {
  kind: "symlink",
  link: { target: "/etc/hosts", targetKind: "file" },
});
const broken = entry("lien-casse", {
  kind: "symlink",
  link: { target: "/nulle-part", targetKind: "broken" },
});

describe("genre affiché", () => {
  it("range un lien vers un dossier avec les dossiers", () => {
    expect(isDirLike(entry("d", { kind: "dir" }))).toBe(true);
    expect(isDirLike(dirLink)).toBe(true);
    expect(isDirLike(fileLink)).toBe(false);
  });

  it("autorise le téléchargement d'un fichier ou d'un lien vers un fichier", () => {
    expect(isFileLike(entry("f"))).toBe(true);
    expect(isFileLike(fileLink)).toBe(true);
    expect(isFileLike(broken)).toBe(false);
    expect(isFileLike(entry("s", { kind: "other" }))).toBe(false);
  });
});

describe("chemins", () => {
  it("découpe et reconstruit un chemin absolu", () => {
    expect(pathSegments("/home/deploy/")).toEqual(["home", "deploy"]);
    expect(pathSegments("/")).toEqual([]);
    expect(pathFromSegments(["home", "deploy"])).toBe("/home/deploy");
    expect(pathFromSegments([])).toBe("/");
  });

  it("extrait le nom d'un fichier local Windows ou Unix", () => {
    expect(localBaseName("C:\\Users\\moi\\a.tar")).toBe("a.tar");
    expect(localBaseName("/home/moi/b.sql.gz")).toBe("b.sql.gz");
  });
});

describe("sortEntries", () => {
  const entries = [
    entry("b.txt", { size: 10, modified: 3 }),
    entry("dossier", { kind: "dir", size: 4096, modified: 1 }),
    entry("A.txt", { size: 30, modified: 2 }),
    dirLink,
    entry("fichier10", { size: 20 }),
    entry("fichier9", { size: 20 }),
  ];
  const names = (list: FileEntry[]) => list.map((e) => e.name);

  it("met les dossiers en premier, puis l'ordre naturel sans casse", () => {
    expect(
      names(sortEntries(entries, { key: "name", descending: false })),
    ).toEqual([
      "dossier",
      "lien-dossier",
      "A.txt",
      "b.txt",
      "fichier9",
      "fichier10",
    ]);
  });

  it("garde les dossiers en tête dans l'ordre décroissant", () => {
    const sorted = sortEntries(entries, { key: "name", descending: true });
    expect(names(sorted).slice(0, 2)).toEqual(["lien-dossier", "dossier"]);
  });

  it("trie les fichiers par taille, les dossiers restant par nom", () => {
    expect(
      names(sortEntries(entries, { key: "size", descending: false })),
    ).toEqual([
      "dossier",
      "lien-dossier",
      "b.txt",
      "fichier9",
      "fichier10",
      "A.txt",
    ]);
  });

  it("ne modifie pas la liste reçue", () => {
    const copy = [...entries];
    sortEntries(entries, { key: "modified", descending: true });
    expect(entries).toEqual(copy);
  });
});
