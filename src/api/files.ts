import { Channel } from "@tauri-apps/api/core";
import type { DirSummary } from "../bindings/DirSummary";
import type { FileEntry } from "../bindings/FileEntry";
import type { Progress } from "../bindings/Progress";
import { call } from "./invoke";

// Explorateur de fichiers SFTP (docs/04-contrat-commandes.md). Les chemins
// distants sont absolus ; ils sont validés par le backend.

/** Dossier personnel du compte SSH, chemin absolu résolu. */
export function homeDir(): Promise<string> {
  return call("home_dir");
}

/** Contenu d'un dossier, dans l'ordre du serveur. */
export function listDir(path: string): Promise<FileEntry[]> {
  return call("list_dir", { path });
}

/** Crée un fichier vide ; ne remplace jamais un élément existant. */
export function createFile(path: string): Promise<void> {
  return call("create_file", { path });
}

/** Crée un dossier ; ne remplace jamais un élément existant. */
export function createDir(path: string): Promise<void> {
  return call("create_dir", { path });
}

/** Renomme un élément ; la destination ne doit pas exister. */
export function rename(fromPath: string, toPath: string): Promise<void> {
  return call("rename", { fromPath, toPath });
}

/** Contenu d'un dossier (ou d'un élément seul), affiché avant sa suppression. */
export function dirSummary(path: string): Promise<DirSummary> {
  return call("dir_summary", { path });
}

/**
 * Supprime un élément. Un lien est supprimé seul, jamais sa cible ; un
 * dossier non vide demande `recursive`.
 */
export function deletePath(path: string, recursive: boolean): Promise<void> {
  return call("delete_path", { path, recursive });
}

/**
 * Lance l'envoi d'un fichier local. Renvoie l'identifiant du transfert ; la
 * progression et l'issue arrivent par `onProgress`. Sans `overwrite`, une
 * cible existante est refusée (`AlreadyExists`).
 */
export function upload(
  localPath: string,
  remotePath: string,
  overwrite: boolean,
  onProgress: (event: Progress) => void,
): Promise<string> {
  const channel = new Channel<Progress>();
  channel.onmessage = onProgress;
  return call("upload", {
    localPath,
    remotePath,
    overwrite,
    onProgress: channel,
  });
}

/** Lance le téléchargement d'un fichier distant vers `localPath`. */
export function download(
  remotePath: string,
  localPath: string,
  onProgress: (event: Progress) => void,
): Promise<string> {
  const channel = new Channel<Progress>();
  channel.onmessage = onProgress;
  return call("download", { remotePath, localPath, onProgress: channel });
}

/** Annule un transfert ; rend la main une fois son temporaire supprimé. */
export function cancelTransfer(transferId: string): Promise<void> {
  return call("transfer_cancel", { transferId });
}
