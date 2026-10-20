/**
 * Validation instantanée du formulaire de profil : champs requis et port,
 * comme `config::normalize`. Le backend contrôle le reste (longueur du nom,
 * format de l'hôte et de l'utilisateur) et fait foi.
 */
import type { Profile } from "./bindings/Profile";
import { trimLikeRust } from "./text";

export type ProfileFields = Pick<
  Profile,
  "name" | "host" | "port" | "user" | "keyPath"
>;

export type ProfileProblems = Record<keyof ProfileFields, string | null>;

function required(value: string, message: string): string | null {
  return trimLikeRust(value) === "" ? message : null;
}

export function profileProblems(profile: ProfileFields): ProfileProblems {
  return {
    name: required(profile.name, "Le nom du profil est requis."),
    host: required(profile.host, "L'hôte est requis."),
    port:
      Number.isInteger(profile.port) &&
      profile.port >= 1 &&
      profile.port <= 65535
        ? null
        : "Le port doit être compris entre 1 et 65535.",
    user: required(profile.user, "L'utilisateur est requis."),
    keyPath: required(
      profile.keyPath,
      "Le chemin de la clé privée est requis.",
    ),
  };
}
