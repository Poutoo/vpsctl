<script setup lang="ts">
/**
 * Éditeur de fichier texte (maquettes `docs/design/editeur/`), à la place de
 * la liste dans l'écran Fichiers ; l'itinéraire garde le chemin. Ouverture,
 * refus, lecture seule, édition, enregistrement (Ctrl+S), conflit et vue
 * Différence. Toute sortie avec des modifications non enregistrées passe
 * par un GO / NO GO.
 */
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate } from "vue-router";
import { errorMessage } from "../../api/errors";
import {
  useEditorActions,
  editCounts,
} from "../../composables/useEditorActions";
import { useElapsed } from "../../composables/useElapsed";
import { useNotify } from "../../composables/useNotify";
import { useTransferActions } from "../../composables/useTransferActions";
import type { BlockAction } from "../../editorDiff";
import { readOnlyFacts, readOnlyText, refusalTag } from "../../editorFacts";
import { lineTotal, savedSize } from "../../editorText";
import { formatBytes } from "../../format";
import { timeLabel } from "../../linkText";
import { pathSegments } from "../../remoteFiles";
import { useConnectionStore } from "../../stores/connection";
import { useEditorStore } from "../../stores/editor";
import {
  changedLines,
  diffLines,
  lineCounts,
  splitLines,
} from "../../textDiff";
import PageHeader from "../layout/PageHeader.vue";
import AppButton from "../ui/AppButton.vue";
import StateMessage from "../ui/StateMessage.vue";
import StatusIcon from "../ui/StatusIcon.vue";
import CodeArea from "./CodeArea.vue";
import EditorDiff from "./EditorDiff.vue";
import EditorRefusal from "./EditorRefusal.vue";
import FileItinerary from "./FileItinerary.vue";

const props = defineProps<{ path: string; server: string }>();
const emit = defineEmits<{ navigate: [path: string]; close: [] }>();

const editor = useEditorStore();
const connection = useConnectionStore();
const actions = useEditorActions();
const transferActions = useTransferActions();
const { notify } = useNotify();

const code = ref<InstanceType<typeof CodeArea> | null>(null);
const cursor = ref({ line: 1, column: 1 });
const escapeArmed = ref(false);

watch(
  () => props.path,
  (path) => {
    cursor.value = { line: 1, column: 1 };
    void editor.open(path);
  },
  { immediate: true },
);

onBeforeUnmount(() => editor.clear());

// Liaison rétablie avec des modifications : l'enregistrement revérifiera
// le fichier (maquette 16).
const stopRestored = connection.on("restored", () => {
  if (editor.dirty) {
    notify({
      tone: "success",
      title: "Liaison rétablie",
      detail: `AOS · ${props.server} · fichier à revérifier`,
    });
  }
});
onBeforeUnmount(stopRestored);

// Toute sortie de l'éditeur (autre écran, autre chemin, Fermer, étape de
// l'itinéraire) : GO / NO GO s'il reste des modifications.
onBeforeRouteLeave(() => actions.confirmExit("close"));
onBeforeRouteUpdate((to, from) =>
  to.fullPath === from.fullPath ? true : actions.confirmExit("close"),
);

const name = computed(() => {
  const segments = pathSegments(props.path);
  return segments[segments.length - 1] ?? props.path;
});
const doc = computed(() => editor.document);

// Fichier ouvert : le curseur est placé dans le texte.
watch(doc, (current, previous) => {
  if (current && current !== previous) {
    void nextTick(() => code.value?.focus());
  }
});

// --- Modifications (calcul différé après la frappe) ---------------------------

const counts = ref({ added: 0, removed: 0 });
const changed = ref<ReadonlySet<number>>(new Set());
const size = ref<number | null>(null);
let timer: ReturnType<typeof setTimeout> | null = null;

function recount() {
  timer = null;
  const current = doc.value;
  if (!current) return;
  const hunks = diffLines(splitLines(editor.base), splitLines(editor.text));
  counts.value = lineCounts(hunks);
  changed.value = changedLines(hunks);
  size.value = savedSize(editor.text, current.bom, current.lineEnding);
}

watch(
  () => [editor.text, editor.base, doc.value] as const,
  () => {
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(recount, 150);
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  if (timer !== null) clearTimeout(timer);
});

// --- Ctrl+S --------------------------------------------------------------------

/**
 * Ctrl+S est toujours intercepté pendant l'édition (phase de capture), pour
 * qu'aucun comportement du moteur web ne s'y attache, puis enregistre si
 * c'est possible. Ignoré sous une fenêtre GO / NO GO.
 */
function onKeydown(event: KeyboardEvent) {
  if (
    (event.ctrlKey || event.metaKey) &&
    !event.altKey &&
    !event.shiftKey &&
    event.key.toLowerCase() === "s"
  ) {
    event.preventDefault();
    event.stopPropagation();
    if (document.querySelector("[role='alertdialog']")) return;
    void actions.save();
  }
}

onMounted(() => window.addEventListener("keydown", onKeydown, true));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown, true));

// --- États affichés -----------------------------------------------------------

const savingElapsed = useElapsed(computed(() => editor.savingSince));

const reasons = computed(() => doc.value?.readOnly ?? []);
const readOnly = computed(() => reasons.value.length > 0);
const readOnlyNotice = computed(() => {
  const current = doc.value;
  if (!current || reasons.value.length === 0) return null;
  return {
    ...readOnlyText(reasons.value[0], current.file),
    facts: readOnlyFacts(current.file, reasons.value),
    others: reasons.value
      .slice(1)
      .map((r) => readOnlyText(r, current.file).short),
  };
});

const countsLabel = computed(() =>
  editCounts(counts.value.added, counts.value.removed),
);

/** Bandeau du cadre : état à gauche, précision à droite. */
const bar = computed(() => {
  const current = doc.value;
  const opened = current ? `ouvert à ${timeLabel(current.openedAt)}` : "";
  if (current && readOnly.value) {
    return {
      tone: "muted",
      label: "Lecture seule",
      icon: null,
      detail: `${readOnlyText(reasons.value[0], current.file).short} · ${opened}`,
    };
  }
  if (connection.lost) {
    return {
      tone: "danger",
      icon: "triangle" as const,
      label: editor.dirty
        ? `Liaison perdue · ${countsLabel.value}`
        : "Liaison perdue",
      detail: "texte conservé · enregistrement indisponible",
    };
  }
  if (savingElapsed.value !== null) {
    return {
      tone: "warning",
      icon: "half" as const,
      label: `Enregistrement · T+${(savingElapsed.value / 1000).toLocaleString("fr-FR", { minimumFractionDigits: 1, maximumFractionDigits: 1 })} s`,
      detail: "écriture d'un temporaire puis renommage",
    };
  }
  if (editor.dirty) {
    return {
      tone: "warning",
      icon: "disc" as const,
      label: `Modifié · ${countsLabel.value}`,
      detail: `Ctrl+S enregistre · ${opened}`,
    };
  }
  if (editor.saved) {
    return {
      tone: "success",
      icon: "check" as const,
      label: `Enregistré à ${timeLabel(editor.saved.at)}`,
      detail: editor.saved.verified
        ? "identique au serveur"
        : "modifié sur le serveur juste après l'enregistrement",
    };
  }
  return {
    tone: "muted",
    icon: null,
    label: "Édition",
    detail: `aucune modification · ${opened}`,
  };
});

const lines = computed(() => lineTotal(editor.text));

const statusRight = computed(() => {
  const current = doc.value;
  if (!current) return [];
  const parts = [
    current.bom ? "UTF-8 BOM" : "UTF-8",
    current.lineEnding === "crlf"
      ? "CRLF"
      : current.lineEnding === "mixed"
        ? "CRLF + LF"
        : "LF",
    size.value === null
      ? formatBytes(current.file.size)
      : formatBytes(size.value),
    `${current.file.permissions} ${current.file.mode ?? ""}`.trim(),
  ];
  if (readOnly.value) parts.push("lecture seule");
  return parts;
});

/** Bandeau permanent des fichiers de ~/.ssh. */
const sshBanner = computed(() => {
  const ssh = doc.value?.sshFile ?? editor.refused?.sshFile ?? null;
  if (ssh === null) return null;
  return ssh === "authorizedKeys"
    ? {
        label: "Fichier d'accès SSH",
        text: `Une erreur ici peut vous empêcher de vous reconnecter à ${props.server}, Atlas compris. Chaque enregistrement passe par un GO / NO GO.`,
      }
    : {
        label: "Fichier SSH",
        text: "Ce fichier règle les connexions SSH de ce compte (~/.ssh). Vérifiez chaque modification avant d'enregistrer.",
      };
});

const tag = computed(() => (editor.refused ? "Refusé" : "Édition"));

const conflictServer = computed(() => editor.conflict?.server ?? null);
/** Version du serveur affichée dans la vue Différence (texte lisible). */
const diffServer = computed(() =>
  editor.view === "diff" ? (conflictServer.value?.text ?? null) : null,
);

/** Action d'un bloc : vos lignes remplacées, annulable par Ctrl+Z. */
function applyBlock(action: BlockAction) {
  code.value?.replace(action.yours[0], action.yours[1], action.replacement);
}

function backToEdit() {
  editor.view = "edit";
  void nextTick(() => code.value?.focus());
}

function download() {
  const file = editor.refused?.file ?? doc.value?.file;
  if (file) void transferActions.download({ name: file.name, path: file.path });
}

watch(
  () => editor.error,
  (error) => {
    if (error && error.kind !== "LinkLost" && editor.document) {
      notify({
        tone: "error",
        title: "Lecture impossible",
        message: errorMessage(error),
      });
    }
  },
);
</script>

<template>
  <div class="editor">
    <PageHeader kicker="II · Itinéraire · Édition" :title="name">
      <template #status>
        <span
          v-if="editor.dirty"
          class="caps pill unsaved"
          data-testid="editor-unsaved"
        >
          <StatusIcon name="disc" /> Non enregistré
        </span>
        <span v-else-if="readOnly" class="caps pill">Lecture seule</span>
      </template>
      <AppButton data-testid="editor-close" @click="emit('close')">
        Fermer
      </AppButton>
      <AppButton
        v-if="doc && !readOnly"
        variant="primary"
        :disabled="!editor.canSave"
        :busy-since="editor.savingSince"
        data-testid="editor-save"
        @click="actions.save()"
      >
        {{ editor.savingSince === null ? "Enregistrer" : "Enregistrement…" }}
        <kbd class="kbd">Ctrl+S</kbd>
      </AppButton>
    </PageHeader>

    <FileItinerary
      :path="path"
      :server="server"
      :busy="connection.lost"
      :file-tag="tag"
      :file-refused="editor.refused !== null"
      @navigate="(p) => emit('navigate', p)"
    />

    <p
      v-if="sshBanner"
      class="ssh-banner"
      role="note"
      data-testid="editor-ssh-banner"
    >
      <StatusIcon class="ssh-shape" name="triangle" />
      <span
        ><span class="caps ssh-label">{{ sshBanner.label }} ·</span>
        {{ sshBanner.text }}</span
      >
    </p>

    <section
      class="frame"
      :class="{ frozen: connection.lost }"
      aria-label="Fichier"
      :aria-busy="editor.loading"
    >
      <StateMessage
        v-if="editor.error && !doc && !editor.refused"
        tone="error"
        kicker="Lecture impossible"
        :kicker-detail="path"
        title="Le fichier n'a pas pu être lu"
        class="message"
      >
        {{ errorMessage(editor.error) }}
        <template #actions>
          <AppButton :disabled="connection.lost" @click="editor.open(path)">
            Réessayer
          </AppButton>
        </template>
      </StateMessage>

      <template v-else-if="editor.refused">
        <header class="bar">
          <span class="caps bar-label danger"
            >Refusé · {{ refusalTag(editor.refused.refusal) }}</span
          >
          <span class="bar-detail">contenu non affiché</span>
        </header>
        <EditorRefusal
          :refused="editor.refused"
          @download="download"
          @back="emit('close')"
        />
        <footer class="status mono">
          <span class="path">{{ path }}</span>
          <span>{{ formatBytes(editor.refused.file.size) }}</span>
          <span
            >{{ editor.refused.file.permissions }}
            {{ editor.refused.file.mode }}</span
          >
        </footer>
      </template>

      <template v-else-if="doc">
        <header class="bar" data-testid="editor-bar">
          <span class="caps bar-label" :class="bar.tone">
            <StatusIcon v-if="bar.icon" :name="bar.icon" />
            {{ bar.label }}
          </span>
          <span class="bar-detail" data-testid="editor-bar-detail">{{
            bar.detail
          }}</span>
        </header>

        <div
          v-if="readOnlyNotice"
          class="notice"
          data-testid="editor-read-only"
        >
          <StatusIcon class="notice-shape" name="circle" />
          <div class="notice-text">
            <p class="caps notice-label">
              Lecture seule · {{ readOnlyNotice.short }}
            </p>
            <p class="notice-title">{{ readOnlyNotice.title }}</p>
            <p class="notice-advice">{{ readOnlyNotice.advice }}</p>
            <p v-if="readOnlyNotice.others.length" class="notice-advice">
              Aussi : {{ readOnlyNotice.others.join(", ") }}.
            </p>
            <p class="mono notice-facts">{{ readOnlyNotice.facts }}</p>
          </div>
          <AppButton size="compact" @click="download">Télécharger</AppButton>
        </div>

        <div class="body">
          <CodeArea
            ref="code"
            :model-value="editor.text"
            :readonly="readOnly"
            :changed="changed"
            :label="`Contenu de ${name}`"
            @update:model-value="editor.setText"
            @cursor="(c) => (cursor = c)"
            @escape="(armed) => (escapeArmed = armed)"
          />
          <EditorDiff
            v-if="diffServer !== null"
            class="diff-layer"
            :base="editor.base"
            :server="diffServer"
            :yours="editor.text"
            :server-modified="conflictServer?.snapshot?.modified ?? null"
            :save-disabled="connection.lost || editor.savingSince !== null"
            @back="backToEdit"
            @save="actions.saveFromDiff()"
            @apply="applyBlock"
          />
        </div>

        <footer class="status mono" data-testid="editor-status">
          <span>Ligne {{ cursor.line }}, colonne {{ cursor.column }}</span>
          <span>{{ lines }} {{ lines > 1 ? "lignes" : "ligne" }}</span>
          <span v-if="escapeArmed" class="hint">Tab : quitter la zone</span>
          <span class="spacer" />
          <span v-for="part in statusRight" :key="part">{{ part }}</span>
        </footer>
      </template>

      <StateMessage
        v-else
        kicker="Lecture"
        :kicker-detail="path"
        title="Ouverture du fichier"
        class="message"
      >
        Lecture par SFTP…
      </StateMessage>
    </section>
  </div>
</template>

<style scoped>
.editor {
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
  height: 100%;
  padding: var(--sp-6);
}

.pill {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-1);
  color: var(--color-text-muted);
}

.pill.unsaved {
  color: var(--color-warning);
}

.kbd {
  margin-left: var(--sp-2);
  padding: 0 var(--sp-1);
  border: var(--border-width) solid currentColor;
  border-radius: var(--r-1);
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
}

.ssh-banner {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-3);
  margin: 0;
  padding: var(--sp-3) var(--sp-4);
  background: var(--color-danger-tint);
  border: var(--border-width) solid var(--color-danger);
  font-size: var(--fs-md);
}

.ssh-shape {
  flex-shrink: 0;
  margin-top: var(--sp-1);
  color: var(--color-danger);
}

.ssh-label {
  color: var(--color-danger-text);
}

.frame {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.frame.frozen {
  border-style: dashed;
  border-color: var(--color-border-strong);
}

.message {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.bar {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-4);
  min-height: var(--row-height);
  padding: 0 var(--sp-4);
  background: var(--color-surface-raised);
  border-bottom: var(--border-width) solid var(--color-border);
}

.bar-label {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  color: var(--color-text-muted);
}

.bar-label.warning {
  color: var(--color-warning);
}

.bar-label.success {
  color: var(--color-success);
}

.bar-label.danger {
  color: var(--color-danger-text);
}

.bar-detail {
  min-width: 0;
  overflow: hidden;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.notice {
  display: flex;
  flex-shrink: 0;
  align-items: flex-start;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
}

.notice-shape {
  flex-shrink: 0;
  margin-top: var(--sp-1);
  color: var(--color-text-muted);
}

.notice-text {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.notice-text p {
  margin: 0;
}

.notice-label {
  color: var(--color-text-muted);
}

.notice-title {
  font-size: var(--fs-md);
  font-weight: 600;
}

.notice-advice {
  color: var(--color-text-muted);
  font-size: var(--fs-md);
}

.notice-facts {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
  overflow-wrap: anywhere;
}

.body {
  position: relative;
  display: flex;
  flex: 1;
  min-height: 0;
}

/* Vue Différence par-dessus la zone d'édition, qui reste montée : son
   historique d'annulation est conservé. */
.diff-layer {
  position: absolute;
  inset: 0;
}

.status {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: var(--sp-4);
  min-height: var(--row-height);
  padding: 0 var(--sp-4);
  background: var(--color-surface-raised);
  border-top: var(--border-width) solid var(--color-border);
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.status .path {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.spacer {
  flex: 1;
}

.hint {
  color: var(--color-action);
}
</style>
