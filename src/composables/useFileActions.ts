import { errorMessage, toAppError } from "../api/errors";
import * as api from "../api/files";
import { copyText } from "../api/local";
import type { DirSummary } from "../bindings/DirSummary";
import type { FileEntry } from "../bindings/FileEntry";
import { formatBytes } from "../format";
import { joinPath, parentPath } from "../remoteFiles";
import { useFilesStore } from "../stores/files";
import { useConfirm, type ConfirmOptions } from "./useConfirm";
import { useNotify } from "./useNotify";

/**
 * Avertissement et case à cocher du GO / NO GO avant une action qui peut
 * couper l'accès SSH (`FileEntry.sshAccess`) : supprimer ou renommer
 * `~/.ssh`, supprimer, renommer ou écraser `~/.ssh/authorized_keys`.
 */
export const SSH_GUARD = {
  warning: "Cette action peut vous faire perdre l'accès SSH à ce serveur.",
  acknowledge: "Je comprends que je peux perdre l'accès SSH à ce serveur",
} satisfies Partial<ConfirmOptions>;

export function sshGuard(entry: FileEntry): Partial<ConfirmOptions> {
  return entry.sshAccess ? SSH_GUARD : {};
}

/** « 1 fichier », « 12 fichiers ». */
function count(value: number, singular: string, plural = `${singular}s`) {
  return `${value.toLocaleString("fr-FR")} ${value > 1 ? plural : singular}`;
}

/** Contenu d'un dossier : « 3 sous-dossiers, 12 fichiers · 4,2 Mo ». */
export function summaryText(summary: DirSummary): string {
  const parts: string[] = [];
  if (summary.dirs > 0) parts.push(count(summary.dirs, "sous-dossier"));
  if (summary.files > 0) parts.push(count(summary.files, "fichier"));
  if (summary.links > 0) {
    parts.push(count(summary.links, "lien symbolique", "liens symboliques"));
  }
  if (parts.length === 0) return "Le dossier est vide";
  return `${parts.join(", ")} · ${formatBytes(summary.bytes)}`;
}

function deleteTitle(entry: FileEntry): string {
  switch (entry.kind) {
    case "dir":
      return "Supprimer le dossier ?";
    case "file":
      return "Supprimer le fichier ?";
    case "symlink":
      return "Supprimer le lien ?";
    case "other":
      return "Supprimer l'élément ?";
  }
}

/** Conséquences listées dans le GO / NO GO d'une suppression. */
function deleteConsequences(
  entry: FileEntry,
  summary: DirSummary | null,
): string[] {
  const lines: string[] = [];
  if (entry.kind === "dir" && summary) {
    lines.push(`Contenu supprimé : ${summaryText(summary)}`);
    if (summary.truncated) {
      lines.push(
        "Plus de 100 000 éléments : le décompte s'est arrêté là, tout le contenu sera supprimé",
      );
    }
    if (summary.links > 0) {
      lines.push(
        "Les liens symboliques sont supprimés seuls, jamais leur cible",
      );
    }
    lines.push("Si un sous-dossier est illisible, rien n'est supprimé");
  } else if (entry.kind === "symlink") {
    const target = entry.link?.target ?? "inconnue";
    lines.push(`Seul le lien est supprimé : sa cible ${target} est conservée`);
  } else if (entry.kind === "file") {
    lines.push(
      `Le fichier (${formatBytes(entry.size)}) est supprimé définitivement`,
    );
  } else {
    lines.push("L'élément est supprimé définitivement");
  }
  if (entry.residue?.kind === "old" && !entry.residue.originalExists) {
    lines.push(
      `Résidu de transfert : c'est probablement la seule copie de « ${entry.residue.original} »`,
    );
  }
  return lines;
}

/** Issue d'une saisie de nom : faite, abandonnée (NO GO) ou refusée. */
export type DraftOutcome = "done" | "cancelled" | { error: string };

/** Actions de l'explorateur, avec leurs confirmations et notifications. */
export function useFileActions() {
  const files = useFilesStore();
  const { confirm } = useConfirm();
  const { notify } = useNotify();

  function fail(title: string, detail: string, raw: unknown) {
    notify({
      tone: "error",
      title,
      detail,
      message: errorMessage(toAppError(raw)),
    });
  }

  /**
   * Suppression : décompte d'un dossier (ligne « en transition »), puis
   * GO / NO GO avec les conséquences, puis suppression.
   */
  async function remove(entry: FileEntry) {
    if (files.busy[entry.path]) return;
    let summary: DirSummary | null = null;
    if (entry.kind === "dir") {
      files.markBusy(entry.path, "Décompte");
      try {
        summary = await api.dirSummary(entry.path);
      } catch (raw) {
        fail("Décompte impossible", entry.path, raw);
        return;
      } finally {
        files.clearBusy(entry.path);
      }
    }
    const confirmed = await confirm({
      title: deleteTitle(entry),
      subject: entry.path,
      consequences: deleteConsequences(entry, summary),
      confirmLabel: "Supprimer",
      danger: true,
      ...sshGuard(entry),
    });
    if (!confirmed) return;

    files.markBusy(entry.path, "Suppression");
    try {
      await api.deletePath(entry.path, entry.kind === "dir");
      notify({ tone: "success", title: "Supprimé", detail: entry.path });
    } catch (raw) {
      fail("Suppression impossible", entry.path, raw);
    } finally {
      files.clearBusy(entry.path);
      await files.refresh();
    }
  }

  /** Crée un fichier ou un dossier dans le dossier affiché, puis le sélectionne. */
  async function create(
    kind: "file" | "dir",
    name: string,
  ): Promise<DraftOutcome> {
    const dir = files.path;
    if (dir === null) return { error: "Aucun dossier affiché." };
    const target = joinPath(dir, name);
    try {
      await (kind === "dir" ? api.createDir(target) : api.createFile(target));
    } catch (raw) {
      return { error: errorMessage(toAppError(raw)) };
    }
    await files.refresh(target);
    return "done";
  }

  /** Renomme dans le même dossier ; GO / NO GO si l'accès SSH est en jeu. */
  async function rename(entry: FileEntry, name: string): Promise<DraftOutcome> {
    const target = joinPath(parentPath(entry.path) ?? "/", name);
    if (entry.sshAccess) {
      const confirmed = await confirm({
        title: "Renommer ?",
        subject: entry.path,
        consequences: [`« ${entry.name} » devient « ${name} »`],
        confirmLabel: "Renommer",
        ...SSH_GUARD,
      });
      if (!confirmed) return "cancelled";
    }
    try {
      await api.rename(entry.path, target);
    } catch (raw) {
      return { error: errorMessage(toAppError(raw)) };
    }
    await files.refresh(target);
    return "done";
  }

  async function copyPath(entry: FileEntry) {
    try {
      await copyText(entry.path);
      notify({ tone: "success", title: "Chemin copié", detail: entry.path });
    } catch {
      notify({
        tone: "error",
        title: "Copie impossible",
        detail: entry.path,
        message: "Le presse-papiers du système n'est pas accessible.",
      });
    }
  }

  return { remove, create, rename, copyPath };
}
