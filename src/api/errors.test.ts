import { describe, expect, it } from "vitest";
import type { AppError } from "../bindings/AppError";
import { errorMessage, toAppError } from "./errors";

describe("toAppError", () => {
  it("garde une AppError telle quelle", () => {
    const error: AppError = { kind: "NotConnected" };
    expect(toAppError(error)).toBe(error);
  });

  it("enveloppe toute autre valeur dans Io", () => {
    expect(toAppError("panne")).toEqual({ kind: "Io", message: "panne" });
    expect(toAppError({ kind: 3 })).toEqual({
      kind: "Io",
      message: "[object Object]",
    });
  });
});

describe("errorMessage", () => {
  it("reprend le message des erreurs qui en portent un", () => {
    expect(errorMessage({ kind: "Network", message: "délai dépassé" })).toBe(
      "délai dépassé",
    );
  });

  it("distingue le côté d'un fichier", () => {
    expect(errorMessage({ kind: "NotFound", path: "/a", side: "remote" })).toBe(
      "Élément introuvable sur le VPS : /a",
    );
    expect(
      errorMessage({ kind: "PermissionDenied", path: "C:\\a", side: "local" }),
    ).toBe("Accès refusé sur cette machine : C:\\a");
  });

  it("détaille l'espace manquant", () => {
    expect(
      errorMessage({
        kind: "InsufficientSpace",
        path: "/srv",
        side: "remote",
        needed: 2048,
        available: 1024,
      }),
    ).toBe(
      "Espace disque insuffisant sur le VPS (2,0 Ko nécessaires, 1,0 Ko libres).",
    );
  });

  it("donne l'étape, la cause puis l'état d'une séquence de service", () => {
    expect(
      errorMessage({
        kind: "ServiceStepFailed",
        unit: "bot.service",
        step: "disable",
        state:
          "Service arrêté, toujours activé au démarrage, fichier en place.",
        cause: { kind: "Remote", exitCode: 1, stderr: "refus" },
      }),
    ).toBe(
      "Arrêt à l'étape « désactivation » : La commande distante a échoué (code 1) : refus. Service arrêté, toujours activé au démarrage, fichier en place.",
    );
  });
});
