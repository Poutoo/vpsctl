import { describe, expect, it } from "vitest";
import profileFields from "../tests/shared/profile-fields.json";
import remoteJoin from "../tests/shared/remote-join.json";
import remoteName from "../tests/shared/remote-name.json";
import remoteParent from "../tests/shared/remote-parent.json";
import scriptFields from "../tests/shared/script-fields.json";
import serviceDescription from "../tests/shared/service-description.json";
import serviceEnv from "../tests/shared/service-env.json";
import serviceExecStart from "../tests/shared/service-exec-start.json";
import serviceName from "../tests/shared/service-name.json";
import serviceWorkingDir from "../tests/shared/service-working-dir.json";
import { profileProblems, type ProfileFields } from "./profileRules";
import {
  joinPath,
  nameProblem as remoteNameProblem,
  parentPath,
} from "./remoteFiles";
import { scriptProblems } from "./scriptRules";
import {
  descriptionProblem,
  envProblems,
  execStartProblem,
  nameProblem,
  workingDirProblem,
  type EnvRow,
} from "./serviceRules";
import { sharedCases, type SharedCase } from "./testUtils";

function noProblem(problems: Record<string, string | null>): boolean {
  return Object.values(problems).every((p) => p === null);
}

/** Un test par cas, nommé par sa justification. */
function verdicts<I>(
  file: { cases: SharedCase[] },
  accepts: (input: I) => boolean,
) {
  it.each(sharedCases(file).map((c) => [c.why, c] as const))(
    "%s",
    (_why, c) => {
      expect(accepts(c.input as I)).toBe(c.valid);
    },
  );
}

function results<I>(
  file: { cases: SharedCase[] },
  transform: (input: I) => unknown,
) {
  it.each(sharedCases(file).map((c) => [c.why, c] as const))(
    "%s",
    (_why, c) => {
      expect(transform(c.input as I)).toEqual(c.expected);
    },
  );
}

describe("cas partagés : création d'un service", () => {
  describe("nom", () => {
    verdicts<string>(serviceName, (v) => nameProblem(v) === null);
  });
  describe("description", () => {
    verdicts<string>(serviceDescription, (v) => descriptionProblem(v) === null);
  });
  describe("dossier de travail", () => {
    verdicts<string>(serviceWorkingDir, (v) => workingDirProblem(v) === null);
  });
  describe("commande", () => {
    verdicts<string>(serviceExecStart, (v) => execStartProblem(v) === null);
  });
  describe("variables d'environnement", () => {
    verdicts<EnvRow[]>(serviceEnv, (rows) => {
      const problems = envProblems(rows);
      return problems.list === null && problems.rows.every(noProblem);
    });
  });
});

describe("cas partagés : explorateur", () => {
  describe("nom saisi", () => {
    verdicts<string>(remoteName, (v) => remoteNameProblem(v) === null);
  });
  describe("jointure", () => {
    results<{ dir: string; name: string }>(remoteJoin, (i) =>
      joinPath(i.dir, i.name),
    );
  });
  describe("dossier parent", () => {
    results<string>(remoteParent, parentPath);
  });
});

describe("cas partagés : scripts et profils", () => {
  describe("script", () => {
    verdicts<{ name: string; command: string }>(scriptFields, (s) =>
      noProblem({ ...scriptProblems(s) }),
    );
  });
  describe("profil", () => {
    verdicts<ProfileFields>(profileFields, (p) =>
      noProblem(profileProblems(p)),
    );
  });
});
