<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { errorMessage } from "../api/errors";
import { useConnectionStore } from "../stores/connection";
import { useProfilesStore } from "../stores/profiles";

const router = useRouter();
const connection = useConnectionStore();
const profiles = useProfilesStore();
const selectedId = ref<string | null>(null);

onMounted(async () => {
  await profiles.ensureLoaded();
  selectedId.value = profiles.preferred?.id ?? null;
});

watch(
  () => connection.phase,
  (phase) => {
    if (phase === "connected") void router.push({ name: "dashboard" });
  },
);

function connect() {
  if (selectedId.value) void connection.connect(selectedId.value);
}
</script>

<template>
  <h1>Liaison au serveur</h1>
  <select v-model="selectedId">
    <option v-for="p in profiles.profiles" :key="p.id" :value="p.id">
      {{ p.name }}
    </option>
  </select>
  <button
    v-if="connection.phase !== 'connecting'"
    type="button"
    :disabled="!selectedId"
    @click="connect"
  >
    Établir la liaison
  </button>
  <button v-else type="button" @click="connection.cancel()">Interrompre</button>
  <ul>
    <li v-for="(s, i) in connection.steps" :key="i">
      {{ s.step.step }} · {{ Math.round(s.atMs) }} ms
    </li>
  </ul>
  <p v-if="connection.error">{{ errorMessage(connection.error) }}</p>
</template>
