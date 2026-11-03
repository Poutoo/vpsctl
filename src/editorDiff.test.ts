import { describe, expect, it } from "vitest";
import {
  buildDiff,
  conflictLosses,
  editSummary,
  envName,
  isEnvFile,
  lineRanges,
  nameList,
  type DiffRow,
} from "./editorDiff";

// Exemple de la maquette 07 (fichier raccourci).
const BASE = [
  "# Base",
  "POSTGRES_DB=urbanflow",
  "POSTGRES_PASSWORD=Hk3",
  "",
  "# Journaux",
  "LOG_LEVEL=info",
  "LOG_FORMAT=json",
  "",
  "# Courriel",
  "SMTP_PORT=587",
  "",
].join("\n");
const SERVER = [
  "# Base",
  "POSTGRES_DB=urbanflow",
  "POSTGRES_PASSWORD=Vq7",
  "",
  "# Journaux",
  "LOG_LEVEL=info",
  "LOG_FORMAT=json",
  "",
  "# Courriel",
  "SMTP_PORT=587",
  "",
  "# Suivi des erreurs",
  "SENTRY_DSN=https://x",
  "",
].join("\n");
const YOURS = [
  "# Base",
  "POSTGRES_DB=urbanflow",
  "POSTGRES_PASSWORD=Hk3",
  "",
  "# Journaux",
  "LOG_LEVEL=debug",
  "LOG_FORMAT=json",
  "FEATURE_ZONES_V2=true",
  "",
  "# Courriel",
  "SMTP_PORT=587",
  "",
].join("\n");

function changed(rows: DiffRow[]) {
  return rows.map((r) =>
    r.type === "same"
      ? `= ${r.text}`
      : `${r.type === "removed" ? "−" : "+"} ${r.text} ${r.who}`,
  );
}

describe("buildDiff", () => {
  it("attribue les lignes au serveur ou à vous et propose une action par bloc", () => {
    const view = buildDiff(BASE, SERVER, YOURS);
    expect(view.blocks.map((b) => b.kind)).toEqual([
      "server",
      "yours",
      "yours",
      "server",
    ]);

    const [password, log, feature, sentry] = view.blocks;
    expect(changed(password.rows)).toEqual([
      "− POSTGRES_PASSWORD=Vq7 server",
      "+ POSTGRES_PASSWORD=Hk3 server",
    ]);
    expect(password.action?.kind).toBe("take");
    expect(password.action?.label).toBe("Prendre la version du serveur");

    expect(changed(log.rows)).toEqual([
      "− LOG_LEVEL=info yours",
      "+ LOG_LEVEL=debug yours",
    ]);
    expect(log.action).toBeNull();
    expect(changed(feature.rows)).toEqual(["+ FEATURE_ZONES_V2=true yours"]);

    expect(sentry.action?.kind).toBe("restore");
    expect(sentry.action?.replacement).toEqual([
      "",
      "# Suivi des erreurs",
      "SENTRY_DSN=https://x",
    ]);
  });

  it("numérote chaque ligne dans sa version et replie les lignes identiques", () => {
    const view = buildDiff(BASE, SERVER, YOURS);
    expect(view.items[0]).toMatchObject({ type: "row" });
    const six = "1\n2\n3\n4\n5\n6\n";
    expect(buildDiff(six, six.replace("6", "X"), six).items[0]).toEqual({
      type: "gap",
      count: 3,
    });
    const removed = view.blocks[0].rows[0];
    expect(removed).toMatchObject({ type: "removed", server: 3 });
    expect(view.blocks[2].rows[0]).toMatchObject({ type: "added", yours: 8 });
    const logIndex = view.items.findIndex(
      (item) => item.type === "block" && item.block === view.blocks[1],
    );
    expect(view.items[logIndex + 1]).toMatchObject({
      type: "row",
      row: { type: "same", text: "LOG_FORMAT=json", server: 7, yours: 7 },
    });
  });

  it("propose Retirer pour une ligne supprimée sur le serveur", () => {
    const view = buildDiff("a\nb\nc\n", "a\nc\n", "a\nb\nc\n");
    expect(view.blocks[0].action?.kind).toBe("remove");
    expect(changed(view.blocks[0].rows)).toEqual(["+ b server"]);
  });

  it("annonce les lignes à vous qu'efface l'action d'un bloc mixte", () => {
    const base = "A=1\nB=2\nC=3\n";
    const server = "A=1\nB=server\nC=3\n";
    const yours = "A=1\nB=mine\nX=new\nC=3\n";
    const [block] = buildDiff(base, server, yours).blocks;
    expect(block.kind).toBe("mixed");
    expect(block.action).toMatchObject({
      kind: "takeMixed",
      erases: [2, 3],
      label: "Prendre la version du serveur · efface vos lignes 2–3",
      yours: [1, 3],
      replacement: ["B=server"],
    });
    expect(changed(block.rows)).toEqual([
      "− B=server server",
      "+ B=mine yours",
      "+ X=new yours",
    ]);
  });

  it("mixte sans ligne à vous à effacer : il rétablit vos suppressions", () => {
    const [block] = buildDiff("A\nB\nC\n", "A\nB2\nC\n", "A\nC\n").blocks;
    expect(block.kind).toBe("mixed");
    expect(block.action?.label).toContain("rétablit");
  });

  it("ne montre aucun bloc quand les deux versions sont identiques", () => {
    expect(buildDiff("a\n", "b\n", "b\n").blocks).toEqual([]);
  });
});

describe("résumés", () => {
  it("regroupe les numéros de ligne", () => {
    expect(lineRanges([9, 3, 7, 8, 3])).toBe("3, 7–9");
  });

  it("reconnaît les fichiers d'environnement", () => {
    for (const name of [".env", ".env.prod", "settings.env"]) {
      expect(isEnvFile(name)).toBe(true);
    }
    expect(isEnvFile("env.txt")).toBe(false);
  });

  it("nomme une ligne de .env par sa clé", () => {
    expect(envName("export DB_URL = x")).toBe("DB_URL");
    expect(envName("# Suivi des erreurs")).toBe("# Suivi des erreurs");
    expect(envName("   ")).toBeNull();
    expect(nameList(["A=1", "A=2", "B=1", "C=1", "D=1", "E=1"])).toBe(
      "A · B · C · D · +1",
    );
  });

  it("dit ce qu'Écraser et Recharger feraient perdre", () => {
    const losses = conflictLosses(buildDiff(BASE, SERVER, YOURS), ".env");
    expect(losses.overwrite.detail).toBe(
      "POSTGRES_PASSWORD · # Suivi des erreurs · SENTRY_DSN",
    );
    expect(losses.reload.detail).toBe("LOG_LEVEL · FEATURE_ZONES_V2");
    expect(losses.reload.lines).toBe(2);
  });

  it("cite les numéros de ligne hors .env", () => {
    const losses = conflictLosses(
      buildDiff("a\nb\nc\nd\n", "a\nB\nc\nd\nE\n", "a\nb\nc\nD\n"),
      "notes.txt",
    );
    expect(losses.overwrite.detail).toBe("lignes 2, 5 du serveur");
    expect(losses.reload.detail).toBe("ligne 4");
  });

  it("résume vos modifications depuis l'ouverture", () => {
    expect(editSummary(BASE, YOURS, ".env")).toEqual({
      added: 2,
      removed: 1,
      detail: "LOG_LEVEL · FEATURE_ZONES_V2",
    });
    expect(editSummary("a\nb\nc\n", "a\nc\n", "notes.txt").detail).toBe(
      "1 ligne retirée",
    );
  });
});
