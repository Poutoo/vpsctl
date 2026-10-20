import { describe, expect, it } from "vitest";
import { causeLabel, outageLabel, timeLabel } from "./linkText";

describe("textes du LOS", () => {
  it("nomme chaque cause et ajoute le message du serveur", () => {
    expect(causeLabel("reset", null)).toBe(
      "connexion réinitialisée par l'hôte",
    );
    expect(causeLabel("timeout", null)).toBe("le serveur ne répond plus");
    expect(causeLabel("closed", "maintenance")).toBe(
      "connexion fermée par le serveur (maintenance)",
    );
    expect(causeLabel("network", null)).toBe("réseau injoignable");
    expect(causeLabel("unknown", null)).toBe("liaison interrompue");
  });

  it("donne la durée de coupure en minutes et secondes", () => {
    expect(outageLabel(17_400)).toBe("00:17");
    expect(outageLabel(93_000)).toBe("01:33");
  });

  it("donne l'heure locale sur 24 h", () => {
    expect(timeLabel(new Date(2026, 9, 20, 9, 41, 7).getTime())).toBe(
      "09:41:07",
    );
  });
});
