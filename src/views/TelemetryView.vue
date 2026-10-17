<script setup lang="ts">
/**
 * Écran Télémétrie : un onglet par flux ouvert (logs ou sortie de script),
 * l'onglet affiché étant celui de l'URL. Les flux appartiennent au store
 * `telemetry` : quitter l'écran ou changer d'onglet ne les interrompt pas.
 * Pas d'entrée propre dans la navigation : la section d'origine du flux
 * affiché y est surlignée.
 */
import { computed, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import StreamConsole from "../components/features/StreamConsole.vue";
import TelemetryTabs from "../components/features/TelemetryTabs.vue";
import PageHeader from "../components/layout/PageHeader.vue";
import AppButton from "../components/ui/AppButton.vue";
import StateMessage from "../components/ui/StateMessage.vue";
import { useScriptGuards } from "../composables/useScriptGuards";
import { useTelemetryStore, type TelemetryTab } from "../stores/telemetry";

const store = useTelemetryStore();
const route = useRoute();
const router = useRouter();
const { confirmCloseTab } = useScriptGuards();

const requestedId = computed(() => {
  const param = route.params.tab;
  return typeof param === "string" && param !== "" ? param : null;
});
const active = computed(() => store.tabById(requestedId.value));

// Onglet absent de l'URL ou fermé : le premier onglet ouvert.
watch(
  [requestedId, active, () => store.tabs.length],
  () => {
    if (route.name !== "telemetry" || active.value) return;
    const first = store.tabs[0];
    if (first) {
      void router.replace({ name: "telemetry", params: { tab: first.id } });
    }
  },
  { immediate: true },
);

/** Fermer l'onglet d'un script en cours l'interrompt : GO / NO GO d'abord. */
async function closeTab(tab: TelemetryTab) {
  if (!(await confirmCloseTab(tab))) return;
  const next = store.closeTab(tab.id);
  if (tab.id !== requestedId.value) return;
  void router.replace(
    next
      ? { name: "telemetry", params: { tab: next.id } }
      : { name: "telemetry" },
  );
}
</script>

<template>
  <div class="telemetry">
    <PageHeader kicker="Télémétrie · Flux de journal" title="Console">
      <TelemetryTabs
        v-if="store.tabs.length > 0"
        :tabs="store.tabs"
        :active-id="active?.id ?? null"
        @close="closeTab"
      />
    </PageHeader>

    <div class="body">
      <StreamConsole v-if="active" :key="active.id" :stream="active" />
      <StateMessage v-else kicker="0 flux" title="Aucun flux ouvert">
        Les logs ouverts en plein écran depuis le tableau de bord et la sortie
        des scripts exécutés s'affichent ici, chacun dans son onglet.
        <template #actions>
          <AppButton :to="{ name: 'dashboard' }">Tableau de bord</AppButton>
          <AppButton :to="{ name: 'scripts' }">Scripts</AppButton>
        </template>
      </StateMessage>
    </div>
  </div>
</template>

<style scoped>
/* Écran plein cadre : la console occupe toute la hauteur sous l'en-tête. */
.telemetry {
  display: flex;
  flex-direction: column;
  gap: var(--sp-5);
  height: 100%;
  padding: var(--sp-6);
}

.body {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}
</style>
