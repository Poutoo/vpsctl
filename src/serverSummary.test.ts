import { describe, expect, it } from "vitest";
import { serverSummary } from "./serverSummary";

describe("serverSummary", () => {
  const base = { os: "Ubuntu 26.04 LTS", docker: "29.6.1", systemd: "259" };

  it("place Compose entre Docker et systemd", () => {
    expect(
      serverSummary({
        ...base,
        compose: { status: "present", version: "5.3.1" },
      }),
    ).toBe("Ubuntu 26.04 LTS · Docker 29.6.1 · Compose 5.3.1 · systemd 259");
  });

  it("signale un plugin absent", () => {
    expect(serverSummary({ ...base, compose: { status: "absent" } })).toBe(
      "Ubuntu 26.04 LTS · Docker 29.6.1 · Compose absent · systemd 259",
    );
  });

  it("n'affiche rien pour une détection sans réponse", () => {
    expect(
      serverSummary({
        os: null,
        docker: null,
        systemd: "259",
        compose: { status: "unknown" },
      }),
    ).toBe("systemd 259");
  });
});
