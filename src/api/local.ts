import { downloadDir, join } from "@tauri-apps/api/path";
import { open, save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

// Machine locale : fenêtres de sélection (plugin dialog), « Afficher » dans
// l'explorateur du système (plugin opener) et presse-papiers.

/** Fichiers à envoyer, choisis par l'utilisateur ; vide si annulé. */
export async function pickFilesToUpload(): Promise<string[]> {
  const picked = await open({
    title: "Fichiers à téléverser",
    multiple: true,
    directory: false,
  });
  if (picked === null) return [];
  return Array.isArray(picked) ? picked : [picked];
}

/**
 * Emplacement d'enregistrement d'un téléchargement, proposé dans le dossier
 * Téléchargements. La fenêtre du système confirme le remplacement d'un
 * fichier existant. `null` si annulé.
 */
export async function pickDownloadTarget(name: string): Promise<string | null> {
  let defaultPath = name;
  try {
    defaultPath = await join(await downloadDir(), name);
  } catch {
    // Dossier Téléchargements introuvable : la fenêtre s'ouvre ailleurs.
  }
  return save({ title: "Enregistrer sous", defaultPath });
}

/** Ouvre le dossier local du fichier, fichier sélectionné. */
export function revealLocalFile(path: string): Promise<void> {
  return revealItemInDir(path);
}

/** Copie un texte dans le presse-papiers du système. */
export function copyText(text: string): Promise<void> {
  return navigator.clipboard.writeText(text);
}
