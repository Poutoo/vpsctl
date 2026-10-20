import { describe, expect, it } from "vitest";
import type { Service } from "../../bindings/Service";
import { startupView } from "./startup";

function service(overrides: Partial<Service>): Service {
  return {
    unit: "bot.service",
    load: "loaded",
    active: "active",
    sub: "running",
    description: null,
    state: "nominal",
    masked: false,
    controllable: true,
    deleteBlock: null,
    startup: "enabled",
    startupBlock: null,
    ...overrides,
  };
}

describe("interrupteur au démarrage", () => {
  it("est actif pour enabled et disabled", () => {
    expect(startupView(service({ startup: "enabled" }))).toEqual({
      checked: true,
      locked: null,
      help: null,
    });
    expect(startupView(service({ startup: "disabled" })).checked).toBe(false);
  });

  it("explique un état non modifiable", () => {
    const view = startupView(
      service({ startup: "static", startupBlock: "otherState" }),
    );
    expect(view.locked).toBe("Statique");
    expect(view.help).toContain("[Install]");
    expect(view.checked).toBe(false);
  });

  it("donne une cause pour chaque blocage du backend", () => {
    const cases = [
      ["invalidName", "Non pilotable"],
      ["masked", "Masqué"],
      ["templateInstance", "Instance"],
      ["linked", "Lié"],
      ["unknown", "Inconnu"],
    ] as const;
    for (const [startupBlock, locked] of cases) {
      const view = startupView(service({ startup: null, startupBlock }));
      expect(view.locked).toBe(locked);
      expect(view.help).not.toBeNull();
    }
  });

  it("montre un état inattendu tel quel", () => {
    const view = startupView(
      service({ startup: "nouveau-mode", startupBlock: "otherState" }),
    );
    expect(view.locked).toBe("nouveau-mode");
    expect(view.help).toContain("nouveau-mode");
  });

  it("place le curseur à droite pour une activation limitée à la session", () => {
    const view = startupView(
      service({ startup: "enabled-runtime", startupBlock: "otherState" }),
    );
    expect(view.checked).toBe(true);
    expect(view.locked).toBe("Activé (session)");
  });
});
