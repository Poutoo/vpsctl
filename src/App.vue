<script setup lang="ts">
import { TooltipProvider } from "reka-ui";
import { watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import AppNav from "./components/layout/AppNav.vue";
import BackgroundLayer from "./components/layout/BackgroundLayer.vue";
import StatusBar from "./components/layout/StatusBar.vue";
import TitleBar from "./components/layout/TitleBar.vue";
import ConfirmDialog from "./components/ui/ConfirmDialog.vue";
import ToastHost from "./components/ui/ToastHost.vue";
import { useConnectionStore } from "./stores/connection";
import { useProfilesStore } from "./stores/profiles";

const connection = useConnectionStore();
const profiles = useProfilesStore();
const route = useRoute();
const router = useRouter();

void profiles.ensureLoaded().catch(() => undefined);

// Liaison perdue ou fermée : retour à l'écran de connexion.
watch(
  () => connection.isConnected,
  (connected) => {
    if (!connected && route.meta.requiresConnection) {
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
        <AppNav v-if="connection.isConnected" />
        <main class="main" :class="{ bleed: route.meta.fullBleed }">
          <RouterView />
        </main>
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

.main {
  flex: 1;
  min-width: 0;
  overflow: auto;
  padding: var(--sp-6);
}

.main.bleed {
  padding: 0;
}
</style>
