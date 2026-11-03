import * as api from "../api/editor";
import { errorMessage, toAppError } from "../api/errors";
import {
  buildDiff,
  conflictLosses,
  editSummary,
  type LossSummary,
} from "../editorDiff";
import { formatBytes } from "../format";
import { timeLabel } from "../linkText";
import { sshConfirmation } from "../sshConfirmation";
import { useConnectionStore } from "../stores/connection";
import { useEditorStore } from "../stores/editor";
import { useProfilesStore } from "../stores/profiles";
import { useConfirm, type ConfirmItem } from "./useConfirm";
import { useNotify } from "./useNotify";

/** Sortie de l'éditeur : fermeture, changement d'écran, liaison, fenêtre. */
export type EditorExit = "close" | "disconnect" | "window";

function plural(count: number, singular: string, pluralForm: string): string {
  return `${count} ${count > 1 ? pluralForm : singular}`;
}

/** « +2 −1 lignes ». */
export function editCounts(added: number, removed: number): string {
  return `+${added} −${removed} ${added + removed > 1 ? "lignes" : "ligne"}`;
}

/**
 * Enregistrement, conflit, différence et sorties de l'éditeur, avec leurs
 * GO / NO GO (maquettes `docs/design/editeur/`).
 */
export function useEditorActions() {
  const editor = useEditorStore();
  const connection = useConnectionStore();
  const profiles = useProfilesStore();
  const { confirm, choose } = useConfirm();
  const { notify } = useNotify();

  const server = () => profiles.byId(connection.profileId)?.name ?? "le VPS";

  /** GO / NO GO avant de perdre les modifications. Sans modification : accepté. */
  async function confirmExit(exit: EditorExit): Promise<boolean> {
    const doc = editor.document;
    if (!doc || !editor.dirty) return true;
    const summary = editSummary(editor.base, editor.text, doc.file.name);
    const kept: ConfirmItem = {
      tone: "ok",
      label: "Le fichier sur le serveur reste tel quel",
      detail:
        editor.versionAt === null
          ? undefined
          : `version du ${timeLabel(editor.versionAt)}`,
    };
    const lost: ConfirmItem = {
      tone: "warn",
      label: `${editCounts(summary.added, summary.removed)} modifiées seront perdues`,
      detail: summary.detail || undefined,
    };
    const titles: Record<EditorExit, [string, string, string]> = {
      close: [
        "Quitter sans enregistrer ?",
        "Les modifications seront perdues.",
        "Quitter sans enregistrer",
      ],
      disconnect: [
        "Couper la liaison sans enregistrer ?",
        `La liaison avec ${server()} sera coupée. Les modifications seront perdues.`,
        "Couper la liaison",
      ],
      window: [
        "Fermer Atlas sans enregistrer ?",
        `La liaison avec ${server()} sera coupée. Les modifications seront perdues.`,
        "Fermer Atlas",
      ],
    };
    const [title, command, confirmLabel] = titles[exit];
    const items: ConfirmItem[] = [lost];
    if (exit === "disconnect") {
      items.push({
        tone: "warn",
        label: `La liaison avec ${server()} sera coupée`,
      });
    } else if (exit === "window") {
      items.push({
        tone: "warn",
        label: `Atlas se ferme et la liaison avec ${server()} est coupée`,
      });
    }
    items.push(kept);
    const go = await confirm({
      title,
      subject: doc.file.path,
      command,
      items,
      consequences: [],
      confirmLabel,
      cancelLabel: "Rester",
      danger: true,
    });
    if (go) editor.discard();
    return go;
  }

  function notifyFailure(raw: unknown) {
    const error = toAppError(raw);
    // Liaison perdue : le bandeau LOS en tient lieu, le texte est conservé.
    if (error.kind === "LinkLost") return;
    notify({
      tone: "error",
      title: "Enregistrement impossible",
      message: `${errorMessage(error)} Rien n'a été écrit ; votre texte est conservé.`,
    });
  }

  /** Enregistre `content` à la place de `expected` ; conflit : sa fenêtre. */
  async function write(
    content: string,
    expected = editor.snapshot,
  ): Promise<void> {
    const doc = editor.document;
    if (!doc) return;
    let outcome;
    try {
      outcome = await editor.save(content, expected);
    } catch (raw) {
      notifyFailure(raw);
      return;
    }
    if (outcome.outcome === "saved") {
      notify({
        tone: "success",
        title: "Enregistré",
        detail: `${doc.file.name} · ${formatBytes(outcome.snapshot.size)} · ${timeLabel(outcome.savedAt)}`,
      });
      return;
    }
    await resolveConflict();
  }

  /** Ctrl+S et bouton Enregistrer. */
  async function save(): Promise<void> {
    const doc = editor.document;
    if (!doc || !editor.canSave) return;
    const content = editor.text;
    if (
      doc.sshFile === "authorizedKeys" &&
      !(await confirmSshSave(editor.base, content))
    ) {
      return;
    }
    await write(content);
  }

  async function reload(): Promise<void> {
    if (editor.path) await editor.open(editor.path);
  }

  /** Remplace la version du serveur vue dans le conflit par votre texte. */
  async function overwrite(): Promise<void> {
    const conflict = editor.conflict;
    const doc = editor.document;
    if (!conflict || !doc || connection.lost) return;
    const content = editor.text;
    // authorized_keys : vérifié par rapport à la version du serveur remplacée.
    if (
      doc.sshFile === "authorizedKeys" &&
      !(await confirmSshSave(conflict.server.text ?? "", content))
    ) {
      return;
    }
    await write(content, conflict.server.snapshot);
  }

  function losses(): { overwrite: LossSummary; reload: LossSummary } | null {
    const conflict = editor.conflict;
    const doc = editor.document;
    if (!conflict || !doc || conflict.server.text === null) return null;
    return conflictLosses(
      buildDiff(editor.base, conflict.server.text, editor.text),
      doc.file.name,
    );
  }

  /** Fenêtre de conflit (Annuler, Recharger, Écraser, différence). */
  async function resolveConflict(): Promise<void> {
    const conflict = editor.conflict;
    const doc = editor.document;
    if (!conflict || !doc) return;
    const { snapshot, refusal } = conflict.server;
    const base = {
      kicker: "Conflit · Confirmation requise",
      aside: "Rien n'a été écrit",
      subject: doc.file.path,
      consequences: [],
      danger: true,
    };
    if (snapshot === null) {
      const go = await confirm({
        ...base,
        title: "Le fichier a été supprimé sur le serveur",
        items: [
          {
            tone: "warn",
            label: "Recréer le fichier avec votre texte",
            detail: `droits de l'ouverture : ${doc.file.mode ?? "644"}`,
          },
        ],
        confirmLabel: "Recréer",
      });
      if (go) await write(editor.text, null);
      return;
    }
    const changed = `Ouvert à ${timeLabel(doc.openedAt)} · modifié sur le serveur${
      snapshot.modified === null ? "" : ` à ${timeLabel(snapshot.modified)}`
    } · ${editor.snapshot?.size ?? "?"} → ${snapshot.size} octets`;
    const loss = losses();
    const items: ConfirmItem[] = loss
      ? [
          {
            tone: "warn",
            label: `Écraser efface ${plural(loss.overwrite.lines, "ligne changée", "lignes changées")} sur le serveur`,
            detail: loss.overwrite.detail || undefined,
          },
          {
            tone: "warn",
            label:
              loss.reload.lines > 1
                ? `Recharger abandonne vos ${loss.reload.lines} lignes modifiées`
                : "Recharger abandonne votre ligne modifiée",
            detail: loss.reload.detail || undefined,
          },
          {
            tone: "neutral",
            label:
              "Pour garder les deux, ouvrez la différence et reprenez les lignes du serveur",
          },
        ]
      : [
          {
            tone: "warn",
            label: "La version du serveur ne s'ouvre plus dans l'éditeur",
            detail: refusal ? refusalShort(refusal.reason) : undefined,
          },
        ];
    const choice = await choose({
      ...base,
      title: "Le fichier a changé sur le serveur depuis son ouverture",
      command: changed,
      items,
      link: loss ? "Voir la différence ligne à ligne →" : undefined,
      alternative: "Recharger la version du serveur",
      confirmLabel: "Écraser",
    });
    if (choice === "go") await overwrite();
    else if (choice === "alternative") await reload();
    else if (choice === "link") editor.view = "diff";
  }

  /** « Enregistrer… » de la vue Différence : écrase en disant ce qui reste perdu. */
  async function saveFromDiff(): Promise<void> {
    const doc = editor.document;
    const loss = losses();
    if (!doc || !loss || connection.lost) return;
    const item: ConfirmItem =
      loss.overwrite.lines === 0
        ? { tone: "ok", label: "Tous les changements du serveur sont repris" }
        : {
            tone: "warn",
            label: `Écraser efface ${plural(loss.overwrite.lines, "ligne changée", "lignes changées")} sur le serveur`,
            detail: loss.overwrite.detail || undefined,
          };
    const go = await confirm({
      kicker: "Conflit · Confirmation requise",
      title: "Remplacer la version du serveur ?",
      subject: doc.file.path,
      items: [item],
      consequences: [],
      confirmLabel: "Enregistrer",
      danger: loss.overwrite.lines > 0,
    });
    if (go) await overwrite();
  }

  /** GO / NO GO de `~/.ssh/authorized_keys`, d'après les vérifications du backend. */
  async function confirmSshSave(
    before: string,
    after: string,
  ): Promise<boolean> {
    const doc = editor.document;
    if (!doc) return false;
    try {
      const check = await api.authorizedKeysCheck(doc.file.path, before, after);
      return await confirm(sshConfirmation(check, doc.file.account));
    } catch (raw) {
      notifyFailure(raw);
      return false;
    }
  }

  return {
    confirmExit,
    save,
    reload,
    overwrite,
    resolveConflict,
    saveFromDiff,
  };
}

function refusalShort(reason: string): string {
  switch (reason) {
    case "tooLarge":
      return "plus de 1 Mo";
    case "binary":
      return "contenu binaire";
    case "invalidUtf8":
      return "UTF-8 invalide";
    case "privateKey":
      return "clé privée";
    case "symlink":
      return "lien symbolique";
    default:
      return "n'est plus un fichier";
  }
}
