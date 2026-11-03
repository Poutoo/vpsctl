import type { AuthorizedKeysCheck } from "../bindings/AuthorizedKeysCheck";
import type { OpenedFile } from "../bindings/OpenedFile";
import type { SaveOutcome } from "../bindings/SaveOutcome";
import type { SaveRequest } from "../bindings/SaveRequest";
import { call } from "./invoke";

// Édition de fichiers texte par SFTP (docs/04-contrat-commandes.md,
// « Édition de fichiers »).

/** Texte du fichier et ce qu'il faut en savoir, ou la raison du refus. */
export function fileOpen(path: string): Promise<OpenedFile> {
  return call("file_open", { path });
}

/**
 * Enregistre si le fichier n'a pas changé depuis `request.expected` ; sinon
 * renvoie le conflit, sans rien écrire.
 */
export function fileSave(request: SaveRequest): Promise<SaveOutcome> {
  return call("file_save", { request });
}

/** Vérifications de `~/.ssh/authorized_keys` avant l'enregistrement. */
export function authorizedKeysCheck(
  path: string,
  before: string,
  after: string,
): Promise<AuthorizedKeysCheck> {
  return call("authorized_keys_check", { path, before, after });
}
