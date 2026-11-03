/**
 * docs/tests-jalon-12.md, section 5 : CRLF, BOM et droits conservés,
 * aucun temporaire restant ; octets relus dans la cible.
 */
import { expect } from "@wdio/globals";
import {
  EDITOR_DIR,
  STATUS,
  bytesOf,
  contentOf,
  openEditor,
  pressSave,
  replaceLine,
  seedEditor,
  typeAtEnd,
  waitBar,
} from "../../support/editor.ts";
import { sh } from "../../support/target.ts";
import { connect, dismissToasts, textOf } from "../../support/ui.ts";

describe("Jalon 12 · 5. Fidélité", () => {
  before(async () => {
    seedEditor();
    await connect();
  });

  afterEach(async () => {
    await dismissToasts();
  });

  it("CRLF conservé", async () => {
    await openEditor(`${EDITOR_DIR}/crlf.env`);
    await replaceLine(2, "LOG_LEVEL=debug");
    await pressSave();
    await waitBar(/^✓? ?ENREGISTRÉ À/i);
    expect(bytesOf(`${EDITOR_DIR}/crlf.env`)).toBe(
      String.raw`N O D E _ E N V = p r o d u c t i o n \r \n P O R T = 8 0 8 0 \r \n L O G _ L E V E L = d e b u g \r \n`,
    );
  });

  it("BOM conservé", async () => {
    await openEditor(`${EDITOR_DIR}/bom.txt`);
    expect(await textOf(STATUS)).toContain("UTF-8 BOM");
    await typeAtEnd("ligne=2");
    await pressSave();
    await waitBar(/ENREGISTRÉ À/i);
    expect(sh(`head -c 3 ${EDITOR_DIR}/bom.txt | od -An -tx1`).trim()).toBe(
      "ef bb bf",
    );
    expect(contentOf(`${EDITOR_DIR}/bom.txt`)).toBe("﻿nom=été\nligne=2");
  });

  it("droits conservés, aucun temporaire restant", async () => {
    await openEditor(`${EDITOR_DIR}/droits.conf`);
    await replaceLine(1, "B=deux");
    await pressSave();
    await waitBar(/ENREGISTRÉ À/i);
    expect(sh(`stat -c %a ${EDITOR_DIR}/droits.conf`).trim()).toBe("640");
    expect(contentOf(`${EDITOR_DIR}/droits.conf`)).toBe("A=1\nB=deux\nC=3\n");
    expect(sh(`ls -A ${EDITOR_DIR} | grep -c atlas- || true`).trim()).toBe("0");
  });

  it("un fichier LF reste en LF", async () => {
    await openEditor(`${EDITOR_DIR}/notes.txt`);
    await replaceLine(0, "UN");
    await pressSave();
    await waitBar(/ENREGISTRÉ À/i);
    expect(sh(`grep -c $'\\r' ${EDITOR_DIR}/notes.txt || true`).trim()).toBe(
      "0",
    );
  });
});
