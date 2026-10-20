import { describe, expect, it } from "vitest";
import {
  filterItems,
  fold,
  matchRanges,
  matchesFilter,
  segments,
} from "./listFilter";

describe("repli du texte comparé", () => {
  it("ignore la casse et les accents", () => {
    expect(fold("Été À Noël")).toBe("ete a noel");
    expect(fold("ÇA")).toBe("ca");
  });

  it("replie aussi une lettre accentuée déjà décomposée", () => {
    expect(fold("éte")).toBe("ete");
  });
});

describe("correspondance", () => {
  it("trouve une sous-chaîne sans casse ni accents, dans les deux sens", () => {
    expect(matchesFilter(["Sauvegarde réseau"], "RESEAU")).toBe(true);
    expect(matchesFilter(["api-reseau"], "réseau")).toBe(true);
    expect(matchesFilter(["urbanflow-api-1"], "flow-api")).toBe(true);
    expect(matchesFilter(["nginx"], "redis")).toBe(false);
  });

  it("ignore les espaces de bord de la recherche", () => {
    expect(matchesFilter(["redis-cache"], "  redis ")).toBe(true);
  });

  it("garde tout quand la recherche est vide", () => {
    expect(matchesFilter(["a"], "")).toBe(true);
    expect(matchesFilter(["a"], "   ")).toBe(true);
  });

  it("ignore un champ absent", () => {
    expect(matchesFilter(["api.service", null], "api")).toBe(true);
    expect(matchesFilter([null], "api")).toBe(false);
  });

  it("filtre une liste en gardant l'ordre", () => {
    const items = [
      { name: "web", image: "nginx:1.27" },
      { name: "db", image: "postgres" },
      { name: "nginx-proxy", image: "caddy" },
    ];
    const shown = filterItems(items, "NGINX", (i) => [i.name, i.image]);
    expect(shown.map((i) => i.name)).toEqual(["web", "nginx-proxy"]);
  });
});

describe("surlignage", () => {
  it("rapporte les plages sur le texte d'origine accentué", () => {
    expect(matchRanges("Fête de l'été", "ete")).toEqual([
      { start: 1, end: 4 },
      { start: 10, end: 13 },
    ]);
  });

  it("couvre une lettre décomposée entière (lettre et accent)", () => {
    const text = "café noir";
    const [range] = matchRanges(text, "café");
    expect(text.slice(range.start, range.end)).toBe("café");
  });

  it("découpe le texte en morceaux surlignés", () => {
    expect(segments("Bot Discord", "disc")).toEqual([
      { text: "Bot ", match: false },
      { text: "Disc", match: true },
      { text: "ord", match: false },
    ]);
  });

  it("garde le texte entier sans correspondance ni recherche", () => {
    expect(segments("api", "zzz")).toEqual([{ text: "api", match: false }]);
    expect(segments("api", "")).toEqual([{ text: "api", match: false }]);
  });

  it("ne chevauche pas deux correspondances", () => {
    expect(matchRanges("aaaa", "aa")).toEqual([{ start: 0, end: 4 }]);
  });
});
