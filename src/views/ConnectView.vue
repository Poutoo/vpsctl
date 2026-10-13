<script setup lang="ts">
import { useRouter } from "vue-router";
import { useConnectionStore } from "../stores/connection";

const router = useRouter();
const connection = useConnectionStore();

// TODO(jalon 1) : supprimer, remplacé par la vraie connexion SSH.
async function simulateConnection() {
  connection.isConnected = true;
  await router.push({ name: "dashboard" });
}
</script>

<template>
  <h1>ConnectView</h1>
  <!-- TODO(jalon 1) : bouton temporaire pour naviguer sans connexion SSH. -->
  <button type="button" @click="simulateConnection">
    Simuler une connexion
  </button>
</template>
