<script setup lang="ts">
import { TooltipProvider } from "reka-ui";
import { onBeforeUnmount, onMounted, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { guardWindowClose } from "./api/window";
import AppNav from "./components/layout/AppNav.vue";
import BackgroundLayer from "./components/layout/BackgroundLayer.vue";
import LinkBanner from "./components/layout/LinkBanner.vue";
import StatusBar from "./components/layout/StatusBar.vue";
import TitleBar from "./components/layout/TitleBar.vue";
import ConfirmDialog from "./components/ui/ConfirmDialog.vue";
import ToastHost from "./components/ui/ToastHost.vue";
import { useScriptGuards } from "./composables/useScriptGuards";
import { useConnectionStore } from "./stores/connection";
import { usePreferencesStore } from "./stores/preferences";
import { useProfilesStore } from "./stores/profiles";

const connection = useConnectionStore();
const profiles = useProfilesStore();
const route = useRoute();
const router = useRouter();
const { confirmLeave } = useScriptGuards();

void profiles.ensureLoaded().catch(() => undefined);
void usePreferencesStore().load();

// Fermeture de l'application pendant un script : GO / NO GO, il sera
// interrompu. Sinon, la fenêtre se ferme directement.
let stopGuard: (() => void) | null = null;
let mounted = false;
onMounted(async () => {
  mounted = true;
  const stop = await guardWindowClose(() => confirmLeave("close"));
  if (mounted) stopGuard = stop;
  else stop();
});
onBeforeUnmount(() => {
  mounted = false;
  stopGuard?.();
});

// Session terminée (déconnexion, abandon du LOS) : retour à l'écran de
// connexion. Un LOS garde l'écran affiché.
watch(
  () => connection.hasSession,
  (session) => {
    if (!session && route.meta.requiresConnection) {
      void router.push({ name: "connect" });
    }
  },
);
</script>

<template>
  <TooltipProvider :delay-duration="400">
    <BackgroundLayer />
    <div class="shell">
      <TitleBar />
      <div class="workspace">
        <AppNav v-if="connection.hasSession" />
        <div class="column">
          <LinkBanner v-if="connection.hasSession" />
          <main class="main" :class="{ bleed: route.meta.fullBleed }">
            <RouterView />
          </main>
        </div>
      </div>
      <StatusBar />
    </div>
    <ConfirmDialog />
    <ToastHost />
  </TooltipProvider>
</template>

<style scoped>
.shell {
  position: relative;
  z-index: var(--z-content);
  display: grid;
  /*
   * Colonne à la largeur de la fenêtre, jamais plus : sans minmax(0, …), la
   * colonne implicite s'élargit au contenu le plus large qui ne se replie pas
   * (un long itinéraire de Fichiers), et la barre de titre sort de l'écran.
   */
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: auto minmax(0, 1fr) auto;
  height: 100%;
}

.workspace {
  display: flex;
  min-height: 0;
}

/* Le bandeau de liaison reste au-dessus du contenu, hors défilement. */
.column {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.main {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--sp-6);
}

.main.bleed {
  padding: 0;
}
</style>
