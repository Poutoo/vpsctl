<script setup lang="ts">
/**
 * Explorateur de fichiers du VPS (SFTP). Le chemin affiché est porté par la
 * route (`/files/home/deploy` ; `/files` = racine) ; la première visite part
 * du dossier personnel. Création et renommage se saisissent dans la liste.
 */
import { computed, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { errorMessage, toAppError } from "../api/errors";
import type { FileEntry } from "../bindings/FileEntry";
import FileItinerary from "../components/features/FileItinerary.vue";
import type { FileAction } from "../components/features/FileMenu.vue";
import FileTable, { type Draft } from "../components/features/FileTable.vue";
import TransferPanel from "../components/features/TransferPanel.vue";
import PageHeader from "../components/layout/PageHeader.vue";
import AppButton from "../components/ui/AppButton.vue";
import AppIcon from "../components/ui/AppIcon.vue";
import IconButton from "../components/ui/IconButton.vue";
import StateMessage from "../components/ui/StateMessage.vue";
import { formatElapsed, useElapsed } from "../composables/useElapsed";
import {
  useFileActions,
  type DraftOutcome,
} from "../composables/useFileActions";
import { useNotify } from "../composables/useNotify";
import { useTransferActions } from "../composables/useTransferActions";
import { isDirLike, parentPath, pathFromSegments } from "../remoteFiles";
import { useConnectionStore } from "../stores/connection";
import { filesRoute, useFilesStore } from "../stores/files";
import { useProfilesStore } from "../stores/profiles";
import { useTransfersStore } from "../stores/transfers";

const route = useRoute();
const router = useRouter();
const files = useFilesStore();
const connection = useConnectionStore();
const profiles = useProfilesStore();
const { notify } = useNotify();
const actions = useFileActions();
const transferActions = useTransferActions();
const transfers = useTransfersStore();

const server = computed(
  () => profiles.byId(connection.profileId)?.name ?? "VPS",
);

const routePath = computed(() => {
  const raw = route.params.path;
  const segments = Array.isArray(raw) ? raw : raw ? [raw] : [];
  return pathFromSegments(segments);
});

// Dossier personnel connu même après un rechargement sur une route directe :
// il sert au refus immédiat de supprimer ou renommer le dossier personnel.
void files.ensureHome().catch(() => undefined);

/** Entrée à sélectionner à l'arrivée dans le dossier (retour au parent). */
let selectOnArrival: string | null = null;

watch(
  [routePath, () => route.query.depart],
  async ([path, depart]) => {
    if (depart === "personnel") {
      try {
        await router.replace(filesRoute(await files.ensureHome()));
      } catch (raw) {
        notify({
          tone: "error",
          title: "Dossier personnel introuvable",
          message: errorMessage(toAppError(raw)),
        });
        await router.replace(filesRoute("/"));
      }
      return;
    }
    const select = selectOnArrival;
    selectOnArrival = null;
    await files.open(path, select ?? undefined);
  },
  { immediate: true },
);

/**
 * Ouvre un dossier. Ignoré pendant une lecture : un seul dossier à la fois,
 * la liste affichée étant celle du dossier précédent.
 */
function navigate(path: string) {
  if (files.loading) return;
  if (path === files.path) {
    void files.refresh();
    return;
  }
  void router.push(filesRoute(path));
}

function openEntry(entry: FileEntry) {
  navigate(entry.path);
}

function goUp() {
  if (files.loading) return;
  const current = files.path;
  const parent = current === null ? null : parentPath(current);
  if (parent === null) return;
  selectOnArrival = current;
  navigate(parent);
}

// --- Saisie d'un nom (création, renommage) -------------------------------------

const draft = ref<Draft | null>(null);
const draftError = ref<string | null>(null);
const draftBusy = ref(false);

function startDraft(next: Draft) {
  // Pendant l'ouverture d'un autre dossier, la liste affichée est périmée.
  if (draftBusy.value || files.navigating) return;
  draftError.value = null;
  draft.value = next;
}

function cancelDraft() {
  if (draftBusy.value) return;
  draft.value = null;
  draftError.value = null;
}

async function commitDraft(name: string) {
  const current = draft.value;
  if (!current || draftBusy.value) return;
  draftBusy.value = true;
  draftError.value = null;
  let outcome: DraftOutcome;
  try {
    if (current.mode === "create") {
      outcome = await actions.create(current.kind, name);
    } else {
      const entry = files.entries.find((e) => e.path === current.path);
      outcome = entry
        ? await actions.rename(entry, name)
        : { error: "Cet élément n'existe plus." };
    }
  } finally {
    draftBusy.value = false;
  }
  if (outcome === "done" || outcome === "cancelled") draft.value = null;
  else draftError.value = outcome.error;
}

// Changement de dossier : une saisie en cours n'a plus d'objet.
watch(
  () => files.path,
  () => {
    draft.value = null;
    draftError.value = null;
  },
);

// Entrée renommée disparue de la liste (relue entre-temps) : la saisie n'a
// plus d'objet.
watch(
  () => files.entries,
  (entries) => {
    const current = draft.value;
    if (
      current?.mode === "rename" &&
      !draftBusy.value &&
      !entries.some((entry) => entry.path === current.path)
    ) {
      draft.value = null;
      draftError.value = null;
    }
  },
);

/** Le dossier affiché accepte une création. */
const canCreate = computed(
  () => files.path !== null && files.error === null && !files.loading,
);

// --- Actions d'une entrée ------------------------------------------------------

function onAction(entry: FileEntry, action: FileAction) {
  // Pendant l'ouverture d'un autre dossier, la liste affichée est périmée :
  // seule la copie du chemin reste possible.
  if ((files.navigating || files.busy[entry.path]) && action !== "copyPath") {
    return;
  }
  switch (action) {
    case "open":
      if (isDirLike(entry)) openEntry(entry);
      break;
    case "rename":
      startDraft({ mode: "rename", path: entry.path });
      break;
    case "copyPath":
      void actions.copyPath(entry);
      break;
    case "delete":
      void actions.remove(entry);
      break;
    case "download":
      void transferActions.download(entry);
      break;
  }
}

const loadingElapsed = useElapsed(computed(() => files.loadingSince));

const countLabel = computed(() => {
  const count = files.entries.length;
  return count === 1 ? "1 entrée" : `${count} entrées`;
});

/** Titre de l'erreur de lecture, selon sa cause. */
const errorTitle = computed(() => {
  switch (files.error?.kind) {
    case "PermissionDenied":
      return "Accès refusé";
    case "NotFound":
      return "Dossier introuvable";
    default:
      return "Lecture impossible";
  }
});
</script>

<template>
  <div class="files">
    <PageHeader section="files" title="Fichiers">
      <AppButton
        :disabled="!canCreate"
        @click="startDraft({ mode: 'create', kind: 'dir' })"
      >
        Nouveau dossier
      </AppButton>
      <AppButton
        :disabled="!canCreate"
        @click="startDraft({ mode: 'create', kind: 'file' })"
      >
        Nouveau fichier
      </AppButton>
      <AppButton
        variant="primary"
        :disabled="!canCreate"
        @click="transferActions.upload()"
      >
        <AppIcon name="upload" />
        Téléverser
      </AppButton>
    </PageHeader>

    <FileItinerary
      v-if="files.path !== null"
      :path="files.path"
      :server="server"
      :busy="files.loading"
      @navigate="navigate"
    />

    <section class="listing" aria-label="Dossier" :aria-busy="files.loading">
      <StateMessage
        v-if="files.error"
        tone="error"
        kicker="Lecture impossible"
        :kicker-detail="files.path ?? undefined"
        :title="errorTitle"
        class="message"
      >
        {{ errorMessage(files.error) }}
        <template #actions>
          <AppButton
            v-if="files.path !== null && files.path !== '/'"
            @click="goUp"
          >
            Remonter
          </AppButton>
          <AppButton @click="files.refresh()">Réessayer</AppButton>
        </template>
      </StateMessage>

      <StateMessage
        v-else-if="
          files.loadedPath !== null && files.entries.length === 0 && !draft
        "
        kicker="0 entrée"
        :kicker-detail="files.loadedPath"
        title="Dossier vide"
        class="message"
      >
        Ce dossier ne contient aucun élément, caché ou non.
      </StateMessage>

      <FileTable
        v-else
        class="table"
        :class="{ stale: files.navigating }"
        :entries="files.sorted"
        :selected="files.selected"
        :sort="files.sort"
        :busy="files.busy"
        :list-key="files.loadedPath ?? ''"
        :draft="draft"
        :draft-error="draftError"
        :draft-busy="draftBusy"
        @select="(path) => (files.selected = path)"
        @open="openEntry"
        @up="goUp"
        @sort="files.setSort"
        @action="onAction"
        @commit-draft="commitDraft"
        @cancel-draft="cancelDraft"
        @edit-draft="draftError = null"
      />

      <footer class="status">
        <span class="mono path" :title="files.path ?? ''">{{
          files.path ?? "—"
        }}</span>
        <span v-if="loadingElapsed !== null" class="mono probing" role="status">
          Lecture · {{ formatElapsed(loadingElapsed, "ms") }}
        </span>
        <span v-else-if="!files.error" class="mono count">{{
          countLabel
        }}</span>
        <IconButton
          icon="refresh"
          label="Actualiser le dossier"
          :unavailable="files.loading ? 'lecture en cours' : null"
          @click="files.refresh()"
        />
      </footer>
    </section>

    <TransferPanel v-if="transfers.items.length > 0" />
  </div>
</template>

<style scoped>
/* Écran plein cadre : la liste occupe la hauteur disponible et défile seule. */
.files {
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
  height: 100%;
  padding: var(--sp-6);
}

.listing {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.table {
  flex: 1;
}

/* État vide ou d'erreur : défile dans le cadre si la hauteur manque. */
.message {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

/* Lecture d'un autre dossier : l'ancien contenu reste, atténué. */
.stale {
  opacity: 0.6;
  transition: opacity var(--dur-fast) var(--ease-out);
}

/*
 * Pendant la lecture, l'ancienne liste ne réagit plus à la souris. Sur la
 * zone défilante et non sur son conteneur : Reka UI pose un
 * `pointer-events: auto` en ligne sur ce dernier, qui l'emporterait.
 */
.stale :deep(.scroller) {
  pointer-events: none;
}

.listing[aria-busy="true"] {
  cursor: progress;
}

.status {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: var(--sp-4);
  min-height: var(--row-height);
  padding: 0 var(--sp-2) 0 var(--sp-4);
  background: var(--color-surface-raised);
  border-top: var(--border-width) solid var(--color-border);
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.path {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.probing {
  color: var(--color-warning);
}
</style>
