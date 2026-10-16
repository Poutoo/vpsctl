<script setup lang="ts">
/**
 * Explorateur de fichiers du VPS (SFTP). Le chemin affiché est porté par la
 * route (`/files/home/deploy` ; `/files` = racine) ; la première visite part
 * du dossier personnel.
 */
import { computed, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { errorMessage, toAppError } from "../api/errors";
import type { FileEntry } from "../bindings/FileEntry";
import FileItinerary from "../components/features/FileItinerary.vue";
import FileTable from "../components/features/FileTable.vue";
import PageHeader from "../components/layout/PageHeader.vue";
import AppButton from "../components/ui/AppButton.vue";
import IconButton from "../components/ui/IconButton.vue";
import StateMessage from "../components/ui/StateMessage.vue";
import { formatElapsed, useElapsed } from "../composables/useElapsed";
import { useNotify } from "../composables/useNotify";
import { parentPath, pathFromSegments } from "../remoteFiles";
import { useConnectionStore } from "../stores/connection";
import { filesRoute, useFilesStore } from "../stores/files";
import { useProfilesStore } from "../stores/profiles";

const route = useRoute();
const router = useRouter();
const files = useFilesStore();
const connection = useConnectionStore();
const profiles = useProfilesStore();
const { notify } = useNotify();

const server = computed(
  () => profiles.byId(connection.profileId)?.name ?? "VPS",
);

const routePath = computed(() => {
  const raw = route.params.path;
  const segments = Array.isArray(raw) ? raw : raw ? [raw] : [];
  return pathFromSegments(segments);
});

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

function navigate(path: string) {
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
  const current = files.path;
  const parent = current === null ? null : parentPath(current);
  if (parent === null) return;
  selectOnArrival = current;
  navigate(parent);
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
    <PageHeader section="files" title="Fichiers" />

    <FileItinerary
      v-if="files.path !== null"
      :path="files.path"
      :server="server"
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
        v-else-if="files.loadedPath !== null && files.entries.length === 0"
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
        :class="{ stale: files.loading }"
        :entries="files.sorted"
        :selected="files.selected"
        :sort="files.sort"
        :busy="files.busy"
        :list-key="files.loadedPath ?? ''"
        @select="(path) => (files.selected = path)"
        @open="openEntry"
        @up="goUp"
        @sort="files.setSort"
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
          @click="files.refresh()"
        />
      </footer>
    </section>
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
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.table {
  flex: 1;
}

.message {
  flex: 1;
}

/* Lecture d'un autre dossier : l'ancien contenu reste, atténué. */
.stale {
  opacity: 0.6;
  transition: opacity var(--dur-fast) var(--ease-out);
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
