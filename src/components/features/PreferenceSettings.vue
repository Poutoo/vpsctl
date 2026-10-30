<script setup lang="ts">
/** Onglet Préférences des Réglages (planche VI). */
import { useConnectionStore } from "../../stores/connection";
import PageHeader from "../layout/PageHeader.vue";
import AppButton from "../ui/AppButton.vue";
import BackgroundSettings from "./BackgroundSettings.vue";
import DiscordSettings from "./DiscordSettings.vue";
import SettingsTabs from "./SettingsTabs.vue";

const connection = useConnectionStore();
</script>

<template>
  <div class="preferences">
    <PageHeader kicker="IV · Préférences" title="Réglages">
      <AppButton v-if="!connection.hasSession" :to="{ name: 'connect' }">
        Retour à la connexion
      </AppButton>
      <SettingsTabs current="preferences" />
    </PageHeader>

    <div class="columns">
      <BackgroundSettings />
      <DiscordSettings />
    </div>
  </div>
</template>

<style scoped>
.preferences {
  display: flex;
  flex-direction: column;
  gap: var(--sp-5);
}

.columns {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--sp-5);
  align-items: start;
}
</style>
