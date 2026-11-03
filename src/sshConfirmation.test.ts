import { describe, expect, it } from "vitest";
import type { AuthorizedKeysCheck } from "./bindings/AuthorizedKeysCheck";
import { sshConfirmation, sshState } from "./sshConfirmation";

function check(
  overrides: Partial<AuthorizedKeysCheck> = {},
): AuthorizedKeysCheck {
  return {
    linkKey: {
      algorithm: "ED25519",
      comment: "deploy@atlas-poste",
      tail: "…V5gT0hPq",
    },
    before: { line: 2, options: [] },
    after: { line: 2, options: [] },
    keys: 2,
    certificates: 0,
    authorities: 0,
    unrecognized: [],
    added: [],
    removed: ["ancien-portable"],
    mode: "600",
    tooOpen: false,
    ...overrides,
  };
}

describe("GO / NO GO d'authorized_keys", () => {
  it("clé de la liaison conservée (maquette 09)", () => {
    const options = sshConfirmation(check(), "deploy");
    expect(sshState(check())).toBe("kept");
    expect(options.title).toBe("Enregistrer un fichier d'accès SSH ?");
    expect(options.aside).toBe("Accès conservé");
    expect(options.command).toBe(
      "Lu par sshd à chaque nouvelle connexion de deploy",
    );
    expect(options.items).toEqual([
      {
        tone: "ok",
        label: "La clé de cette liaison reste autorisée · ligne 2",
        detail: "ED25519 · deploy@atlas-poste · …V5gT0hPq",
      },
      { tone: "ok", label: "2 clés reconnues", detail: undefined },
      { tone: "warn", label: "1 clé retirée", detail: "− ancien-portable" },
      {
        tone: "ok",
        label: "Droits conservés",
        detail: "600 · sshd refuse un fichier trop ouvert",
      },
    ]);
    expect(options.acknowledge).toBeTruthy();
    expect(options.acknowledgeDanger).toBe(false);
    expect(options.alert).toBeUndefined();
  });

  it("clé de la liaison retirée : avertissement fort (maquette 10)", () => {
    const removed = check({ after: null, removed: ["deploy@atlas-poste"] });
    const options = sshConfirmation(removed, "deploy");
    expect(sshState(removed)).toBe("removed");
    expect(options.title).toBe("Enregistrer sans la clé de cette liaison ?");
    expect(options.aside).toBe("Perte d'accès probable");
    expect(options.alert?.label).toBe("Liaison en cours menacée");
    expect(options.items?.[0]).toEqual({
      tone: "danger",
      label: "La clé de cette liaison est retirée du fichier",
      detail:
        "ED25519 · deploy@atlas-poste · …V5gT0hPq · ligne 2 à l'ouverture",
    });
    expect(options.acknowledgeDanger).toBe(true);
  });

  it("clé absente avant comme après : constat neutre (maquette 11)", () => {
    const absent = check({
      before: null,
      after: null,
      added: ["backup-ops"],
      removed: [],
    });
    const options = sshConfirmation(absent, "deploy");
    expect(sshState(absent)).toBe("absent");
    expect(options.aside).toBe("Clé absente avant et après");
    expect(options.items?.[0].tone).toBe("neutral");
    expect(options.items?.[2]).toEqual({
      tone: "warn",
      label: "1 clé ajoutée",
      detail: "+ backup-ops",
    });
    expect(options.acknowledge).toBeTruthy();
    expect(options.acknowledgeDanger).toBe(false);
  });

  it("signale les lignes non reconnues, les certificats et les options", () => {
    const options = sshConfirmation(
      check({
        after: { line: 3, options: ["from", "no-pty"] },
        keys: 2,
        certificates: 1,
        authorities: 1,
        unrecognized: [5, 6],
      }),
      "deploy",
    );
    expect(options.items?.[0].detail).toBe(
      "ED25519 · deploy@atlas-poste · …V5gT0hPq · options : from, no-pty",
    );
    expect(options.items?.[1]).toEqual({
      tone: "warn",
      label: "4 clés reconnues",
      detail:
        "dont 1 certificat · 1 autorité de certification · lignes 5–6 non reconnues par Atlas",
    });
  });

  it("droits trop ouverts", () => {
    const options = sshConfirmation(
      check({ mode: "620", tooOpen: true }),
      "deploy",
    );
    const items = options.items ?? [];
    expect(items[items.length - 1].tone).toBe("danger");
  });
});
