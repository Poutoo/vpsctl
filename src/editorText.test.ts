import { describe, expect, it } from "vitest";
import { lineIndexAt, lineTotal, replaceLines, savedSize } from "./editorText";

describe("texte de l'éditeur", () => {
  it("situe le curseur", () => {
    const text = "ab\ncd\n";
    expect(lineIndexAt(text, 0)).toBe(0);
    expect(lineIndexAt(text, 2)).toBe(0);
    expect(lineIndexAt(text, 3)).toBe(1);
    expect(lineIndexAt(text, 6)).toBe(2);
  });

  it("compte les lignes sans la ligne vide finale", () => {
    expect(lineTotal("")).toBe(0);
    expect(lineTotal("a")).toBe(1);
    expect(lineTotal("a\n")).toBe(1);
    expect(lineTotal("a\nb")).toBe(2);
    expect(lineTotal("a\n\n")).toBe(2);
  });

  it("calcule la taille enregistrée : UTF-8, CRLF et BOM", () => {
    expect(savedSize("é\n", false, "lf")).toBe(3);
    expect(savedSize("é\n", false, "crlf")).toBe(4);
    expect(savedSize("é\n", true, "crlf")).toBe(7);
  });

  it("remplace des lignes par la plus petite modification", () => {
    expect(replaceLines("a\nb\nc\n", 1, 2, ["B"])).toEqual({
      from: 2,
      to: 3,
      insert: "B",
    });
    expect(replaceLines("a\nc\n", 1, 1, ["b"])).toEqual({
      from: 2,
      to: 2,
      insert: "b\n",
    });
    expect(replaceLines("a\nb\nc\n", 1, 2, [])).toEqual({
      from: 2,
      to: 4,
      insert: "",
    });
    expect(replaceLines("a\n", 0, 1, ["a"])).toBeNull();
  });

  it("applique un remplacement en fin de texte sans retour final", () => {
    const text = "a\nb";
    const change = replaceLines(text, 1, 2, ["x", "y"]);
    expect(change).not.toBeNull();
    if (!change) return;
    const result =
      text.slice(0, change.from) + change.insert + text.slice(change.to);
    expect(result).toBe("a\nx\ny");
  });
});
