<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import PreferenceSettings from "../components/features/PreferenceSettings.vue";
import ProfileSettings from "../components/features/ProfileSettings.vue";

const route = useRoute();
const preferences = computed(() => route.params.tab === "preferences");
</script>

<template>
  <PreferenceSettings v-if="preferences" />
  <ProfileSettings v-else />
</template>
