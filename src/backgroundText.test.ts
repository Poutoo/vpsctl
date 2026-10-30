import { describe, expect, it } from "vitest";
import { backgroundDetail, refusalText, refusalTitle } from "./backgroundText";

describe("textes de l'image de fond", () => {
  it("résume taille et dimensions", () => {
    expect(
      backgroundDetail({
        fileName: "nebuleuse-nord.jpg",
        format: "jpeg",
        bytes: 2_516_582,
        width: 3840,
        height: 2160,
      }),
    ).toBe("2,4 Mo · 3840 × 2160");
  });

  it("explique chaque refus en gardant le fond actuel", () => {
    expect(refusalTitle({ kind: "format" })).toBe(
      "Image refusée · format non pris en charge",
    );
    expect(refusalText("capture-ecran.gif", { kind: "format" })).toBe(
      "« capture-ecran.gif » n'est pas lisible. Formats acceptés : PNG, JPEG et WebP. Le fond actuel est conservé.",
    );
    expect(
      refusalText("panorama-8k.png", {
        kind: "tooLarge",
        bytes: 25_794_970,
      }),
    ).toBe(
      "« panorama-8k.png » pèse 24,6 Mo ; la limite est de 10 Mo. Le fond actuel est conservé.",
    );
    const dimensions = refusalText("a.png", {
      kind: "dimensions",
      width: 12_000,
      height: 8_000,
    });
    expect(dimensions.replace(/\s/g, " ")).toBe(
      "« a.png » mesure 12 000 × 8 000 px ; la limite est de 8 192 px de côté et 40 mégapixels. Le fond actuel est conservé.",
    );
    expect(refusalTitle({ kind: "unreadable" })).toBe(
      "Image refusée · image illisible",
    );
    expect(refusalText("a.png", { kind: "unreadable" })).toContain(
      "Le fond actuel est conservé.",
    );
  });
});
