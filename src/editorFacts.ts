/**
 * Textes de l'éditeur tirés des faits du backend (fonctions pures) :
 * propriétaires, raisons de la lecture seule (maquettes 19 à 21).
 */
import type { FileFacts } from "./bindings/FileFacts";
import type { OpenRefusal } from "./bindings/OpenRefusal";
import type { Ownership } from "./bindings/Ownership";
import type { ReadOnlyReason } from "./bindings/ReadOnlyReason";

/** « root:root », ou les identifiants numériques à défaut de noms. */
export function ownerLabel(owner: Ownership): string {
  const user = owner.user ?? (owner.uid === null ? "?" : `${owner.uid}`);
  const group = owner.group ?? (owner.gid === null ? "?" : `${owner.gid}`);
  return `${user}:${group}`;
}

/** Nom seul du propriétaire. */
function userName(owner: Ownership): string {
  return (
    owner.user ??
    (owner.uid === null ? "un compte inconnu" : `l'uid ${owner.uid}`)
  );
}

export interface ReadOnlyText {
  /** Étiquette courte : « droits insuffisants ». */
  short: string;
  title: string;
  advice: string;
}

export function readOnlyText(
  reason: ReadOnlyReason,
  file: FileFacts,
): ReadOnlyText {
  const advice =
    "Le contenu reste lisible et copiable. Pour le modifier, changez d'abord les droits ou le propriétaire sur le serveur.";
  switch (reason) {
    case "permissions":
      return {
        short: "droits insuffisants",
        title: `L'utilisateur ${file.account} n'a pas le droit d'écrire dans ce fichier. L'enregistrement ne pourrait pas aboutir.`,
        advice,
      };
    case "owner":
      return {
        short: "autre propriétaire",
        title: `Ce fichier appartient à ${userName(file.owner)} ; vous êtes connecté en ${file.account}. Enregistrer remplace le fichier par renommage : le nouveau fichier serait à votre nom. Atlas ne change pas le propriétaire d'un fichier.`,
        advice,
      };
    case "directory":
      return {
        short: "dossier non inscriptible",
        title: `Le dossier ${file.dir} appartient à ${userName(file.dirOwner)} et n'est pas inscriptible par ${file.account}. Enregistrer écrit d'abord un fichier temporaire dans ce dossier, ce qui est impossible ici.`,
        advice,
      };
    case "mixedLineEndings":
      return {
        short: "fins de ligne mélangées",
        title:
          "Ce fichier mélange des fins de ligne CRLF et LF (ou contient un CR seul). L'éditeur ne pourrait pas les conserver fidèlement : l'enregistrer les uniformiserait.",
        advice:
          "Le contenu reste lisible et copiable. Pour le modifier ici, uniformisez d'abord ses fins de ligne sur le serveur.",
      };
  }
}

/** Ligne de détail : droits, propriétaire, dossier. */
export function readOnlyFacts(
  file: FileFacts,
  reasons: readonly ReadOnlyReason[],
): string {
  const rights = `${file.permissions} ${file.mode ?? ""}`.trim();
  const dir = reasons.includes("directory")
    ? `dossier ${file.dir} non inscriptible (${file.dirPermissions} ${ownerLabel(file.dirOwner)})`
    : `dossier ${file.dir} inscriptible`;
  return `Droits ${rights} · propriétaire ${ownerLabel(file.owner)} · connecté en ${file.account} · ${dir}`;
}

/** Étiquette courte d'un refus : « Trop volumineux », « Binaire »… */
export function refusalTag(refusal: OpenRefusal): string {
  switch (refusal.reason) {
    case "tooLarge":
      return "Trop volumineux";
    case "binary":
      return "Binaire";
    case "invalidUtf8":
      return "UTF-8 invalide";
    case "privateKey":
      return "Clé privée";
    case "symlink":
      return "Lien symbolique";
    case "notAFile":
      return "Pas un fichier";
  }
}
