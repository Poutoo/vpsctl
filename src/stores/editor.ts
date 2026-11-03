import { defineStore } from "pinia";
import { computed, ref, shallowRef } from "vue";
import * as api from "../api/editor";
import { toAppError } from "../api/errors";
import type { AppError } from "../bindings/AppError";
import type { RefusedFile } from "../bindings/RefusedFile";
import type { SaveOutcome } from "../bindings/SaveOutcome";
import type { ServerVersion } from "../bindings/ServerVersion";
import type { Snapshot } from "../bindings/Snapshot";
import type { TextDocument } from "../bindings/TextDocument";
import { useConnectionStore } from "./connection";

/** Enregistrement terminé : heure et vérification par relecture. */
export interface SavedState {
  at: number;
  verified: boolean;
}

/** Conflit détecté à l'enregistrement : rien n'a été écrit. */
export interface ConflictState {
  server: ServerVersion;
  /** Votre texte au moment du conflit. */
  text: string;
}

/**
 * Fichier ouvert dans l'éditeur (un seul à la fois) : document lu, texte en
 * cours, instantané comparé à l'enregistrement, conflit éventuel. Les
 * fenêtres de confirmation sont dans `useEditorActions`.
 */
export const useEditorStore = defineStore("editor", () => {
  const connection = useConnectionStore();

  const path = ref<string | null>(null);
  const loading = ref(false);
  const error = shallowRef<AppError | null>(null);
  const document = shallowRef<TextDocument | null>(null);
  const refused = shallowRef<RefusedFile | null>(null);
  /** Texte enregistré connu : celui de l'ouverture, puis du dernier enregistrement. */
  const base = ref("");
  const text = ref("");
  const snapshot = shallowRef<Snapshot | null>(null);
  /** Lecture ou dernier enregistrement (ms epoch) : « version du … ». */
  const versionAt = ref<number | null>(null);
  /** Début de l'enregistrement en cours (`performance.now()`). */
  const savingSince = ref<number | null>(null);
  const saved = shallowRef<SavedState | null>(null);
  const conflict = shallowRef<ConflictState | null>(null);
  const view = ref<"edit" | "diff">("edit");

  const dirty = computed(
    () => document.value !== null && text.value !== base.value,
  );
  const readOnly = computed(() => (document.value?.readOnly.length ?? 0) > 0);
  const canSave = computed(
    () =>
      dirty.value &&
      !readOnly.value &&
      !connection.lost &&
      savingSince.value === null &&
      !loading.value,
  );

  let request = 0;

  function clear() {
    request += 1;
    path.value = null;
    loading.value = false;
    error.value = null;
    document.value = null;
    refused.value = null;
    base.value = "";
    text.value = "";
    snapshot.value = null;
    versionAt.value = null;
    savingSince.value = null;
    saved.value = null;
    conflict.value = null;
    view.value = "edit";
  }

  /** Lit le fichier (ouverture ou rechargement) ; le texte en cours est remplacé. */
  async function open(file: string): Promise<void> {
    const current = ++request;
    path.value = file;
    loading.value = true;
    error.value = null;
    try {
      const opened = await api.fileOpen(file);
      if (current !== request) return;
      conflict.value = null;
      saved.value = null;
      view.value = "edit";
      if (opened.kind === "refused") {
        document.value = null;
        refused.value = opened;
        return;
      }
      refused.value = null;
      document.value = opened;
      base.value = opened.text;
      text.value = opened.text;
      snapshot.value = opened.snapshot;
      versionAt.value = opened.openedAt;
    } catch (raw) {
      if (current === request) error.value = toAppError(raw);
    } finally {
      if (current === request) loading.value = false;
    }
  }

  function setText(next: string) {
    text.value = next;
    if (saved.value && next !== base.value) saved.value = null;
  }

  /**
   * Enregistre `content` en remplaçant la version `expected` (par défaut
   * celle de l'ouverture). Un conflit est gardé dans `conflict`.
   */
  async function save(
    content: string,
    expected: Snapshot | null = snapshot.value,
  ): Promise<SaveOutcome> {
    const doc = document.value;
    if (!doc) throw toAppError("aucun fichier ouvert");
    const current = request;
    savingSince.value = performance.now();
    try {
      const outcome = await api.fileSave({
        path: doc.file.path,
        text: content,
        expected,
        bom: doc.bom,
        lineEnding: doc.lineEnding,
        mode: Number.parseInt(doc.file.mode ?? "644", 8),
      });
      if (current !== request) return outcome;
      if (outcome.outcome === "saved") {
        base.value = content;
        snapshot.value = outcome.snapshot;
        versionAt.value = outcome.savedAt;
        conflict.value = null;
        view.value = "edit";
        saved.value =
          text.value === content
            ? { at: outcome.savedAt, verified: outcome.verified }
            : null;
      } else {
        conflict.value = { server: outcome.server, text: content };
      }
      return outcome;
    } finally {
      if (current === request) savingSince.value = null;
    }
  }

  /** Abandonne les modifications (sortie confirmée) : plus rien à garder. */
  function discard() {
    text.value = base.value;
  }

  connection.on("ended", clear);

  return {
    path,
    loading,
    error,
    document,
    refused,
    base,
    text,
    snapshot,
    versionAt,
    savingSince,
    saved,
    conflict,
    view,
    dirty,
    readOnly,
    canSave,
    open,
    setText,
    save,
    discard,
    clear,
  };
});
