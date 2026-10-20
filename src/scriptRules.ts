/**
 * Validation instantanée du formulaire de script : champs requis et longueur
 * du nom, comme `scripts::normalize`. Le backend contrôle le reste
 * (caractères de contrôle, taille de la commande) et fait foi.
 */
import type { Script } from "./bindings/Script";
import { charCount, trimLikeRust } from "./text";

export const MAX_SCRIPT_NAME_CHARS = 64;

export interface ScriptProblems {
  name: string | null;
  command: string | null;
}

export function scriptProblems(
  script: Pick<Script, "name" | "command">,
): ScriptProblems {
  const name = trimLikeRust(script.name);
  return {
    name:
      name === ""
        ? "Le nom du script est requis."
        : charCount(name) > MAX_SCRIPT_NAME_CHARS
          ? `${MAX_SCRIPT_NAME_CHARS} caractères au plus.`
          : null,
    command:
      trimLikeRust(script.command) === "" ? "La commande est requise." : null,
  };
}
