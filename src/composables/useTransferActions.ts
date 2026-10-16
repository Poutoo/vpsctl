import { computed } from "vue";
import { errorMessage, toAppError } from "../api/errors";
import { pickDownloadTarget, pickFilesToUpload } from "../api/local";
import type { FileEntry } from "../bindings/FileEntry";
import { formatBytes, formatModified } from "../format";
import { joinPath, localBaseName } from "../remoteFiles";
import { useConnectionStore } from "../stores/connection";
import { useFilesStore } from "../stores/files";
import { useProfilesStore } from "../stores/profiles";
import { useTransfersStore, type Transfer } from "../stores/transfers";
import { useConfirm } from "./useConfirm";
import { sshGuard } from "./useFileActions";
import { useNotify } from "./useNotify";

/**
 * Envoi et téléchargement depuis l'explorateur : fenêtres de sélection,
 * GO / NO GO avant d'écraser un fichier, refus d'un envoi sur un dossier ou
 * un lien, mise en file d'attente.
 */
export function useTransferActions() {
  const files = useFilesStore();
  const transfers = useTransfersStore();
  const connection = useConnectionStore();
  const profiles = useProfilesStore();
  const { confirm } = useConfirm();
  const { notify } = useNotify();

  const server = computed(
    () => profiles.byId(connection.profileId)?.name ?? "VPS",
  );

  /** GO / NO GO avant de remplacer un fichier du VPS. */
  function confirmOverwrite(target: string, existing: FileEntry | null) {
    const consequences = [
      existing
        ? `Le fichier existant (${formatBytes(existing.size)}${
            existing.modified === null
              ? ""
              : `, modifié le ${formatModified(existing.modified)}`
          }) est remplacé`
        : "Le fichier existant est remplacé",
      "Le remplacement n'a lieu qu'à la fin de l'envoi : annulé ou interrompu, l'envoi laisse le fichier existant intact",
      "Les droits du fichier existant sont conservés",
    ];
    return confirm({
      title: "Remplacer le fichier ?",
      subject: target,
      consequences,
      confirmLabel: "Remplacer",
      danger: true,
      ...(existing ? sshGuard(existing) : {}),
    });
  }

  function refuse(title: string, target: string, message: string) {
    notify({ tone: "error", title, detail: target, message });
  }

  /**
   * Fichiers choisis vers le dossier affiché. Un nom déjà présent dans la
   * liste demande un GO pour être remplacé ; un dossier ou un lien du même
   * nom refuse l'envoi (le backend vérifie à nouveau au démarrage).
   */
  async function upload() {
    const dir = files.path;
    if (dir === null || files.error !== null) return;
    let picked: string[];
    try {
      picked = await pickFilesToUpload();
    } catch (raw) {
      notify({
        tone: "error",
        title: "Sélection impossible",
        message: errorMessage(toAppError(raw)),
      });
      return;
    }
    let queued = 0;
    let lastName = "";
    for (const localPath of picked) {
      const name = localBaseName(localPath);
      const target = joinPath(dir, name);
      const existing =
        files.loadedPath === dir
          ? (files.entries.find((entry) => entry.name === name) ?? null)
          : null;
      let overwrite = false;
      if (existing?.kind === "symlink") {
        refuse(
          "Envoi refusé",
          target,
          `${target} est un lien symbolique vers ${existing.link?.target ?? "une cible illisible"} : un envoi ne remplace jamais un lien.`,
        );
        continue;
      } else if (existing?.kind === "dir") {
        refuse(
          "Envoi refusé",
          target,
          "Un dossier porte déjà ce nom : seuls les fichiers peuvent être remplacés.",
        );
        continue;
      } else if (existing?.kind === "other") {
        refuse(
          "Envoi refusé",
          target,
          "Un fichier spécial porte déjà ce nom : il ne peut pas être remplacé.",
        );
        continue;
      } else if (existing) {
        if (!(await confirmOverwrite(target, existing))) continue;
        overwrite = true;
      }
      transfers.enqueue({
        direction: "upload",
        name,
        localPath,
        remotePath: target,
        overwrite,
      });
      queued += 1;
      lastName = name;
    }
    if (queued > 0) {
      notify({
        tone: "success",
        title: queued === 1 ? "Transfert lancé" : `${queued} transferts lancés`,
        detail: `${queued === 1 ? lastName : `${queued} fichiers`} · LOCAL → ${server.value}`,
      });
    }
  }

  /** Téléchargement d'un fichier, vers un emplacement choisi. */
  async function download(entry: FileEntry) {
    let localPath: string | null;
    try {
      localPath = await pickDownloadTarget(entry.name);
    } catch (raw) {
      notify({
        tone: "error",
        title: "Sélection impossible",
        message: errorMessage(toAppError(raw)),
      });
      return;
    }
    if (localPath === null) return;
    transfers.enqueue({
      direction: "download",
      name: entry.name,
      localPath,
      remotePath: entry.path,
      overwrite: false,
    });
    notify({
      tone: "success",
      title: "Transfert lancé",
      detail: `${entry.name} · ${server.value} → LOCAL`,
    });
  }

  /**
   * Réessai. Un envoi refusé parce que la cible existe désormais demande un
   * GO pour la remplacer.
   */
  async function retry(item: Transfer) {
    const error = item.error;
    if (
      item.direction === "upload" &&
      !item.overwrite &&
      error?.kind === "AlreadyExists" &&
      !error.isDir
    ) {
      const existing =
        files.entries.find((entry) => entry.path === item.remotePath) ?? null;
      if (!(await confirmOverwrite(item.remotePath, existing))) return;
      transfers.retry(item.key, true);
      return;
    }
    transfers.retry(item.key);
  }

  return { server, upload, download, retry };
}
