import { describe, expect, it } from "vitest";
import type { PresenceState } from "./bindings/PresenceState";
import { presenceBadge, presenceNotice, previewKicker } from "./presenceText";

const STATES: PresenceState[] = [
  "off",
  "searching",
  "connected",
  "devUnavailable",
  "notConfigured",
];

describe("textes de la présence Discord", () => {
  it("suivent la planche VI", () => {
    expect(presenceBadge("searching")).toBe("Actif · Discord absent");
    expect(presenceNotice("off").title).toBe("Désactivé");
    expect(presenceNotice("devUnavailable").title).toBe(
      "Indisponible en mode développement",
    );
    expect(previewKicker("connected")).toBe("Aperçu · publié sur Discord");
  });

  it("ne proposent « Réessayer » que si Discord est absent", () => {
    for (const state of STATES) {
      expect(presenceNotice(state).retry).toBe(state === "searching");
    }
  });
});
