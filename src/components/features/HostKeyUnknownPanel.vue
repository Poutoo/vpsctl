<script setup lang="ts">
import FingerprintGroups from "./FingerprintGroups.vue";

defineProps<{
  fingerprint: string;
  endpoint: string;
  busy: boolean;
}>();

const emit = defineEmits<{
  accept: [];
  refuse: [];
}>();
</script>

<template>
  <section class="panel" aria-labelledby="unknown-title">
    <p class="kicker">Première connexion · clé inconnue · {{ endpoint }}</p>
    <h2 id="unknown-title">Comparez l'empreinte du serveur</h2>
    <p>
      C'est la première liaison avec cet hôte. Vérifiez cette empreinte par un
      autre canal, par exemple la console de votre hébergeur :
    </p>
    <pre class="command">ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub</pre>
    <p class="muted">
      Une fois acceptée, l'empreinte est mémorisée dans le profil et une
      nouvelle liaison démarre. Toute différence future bloquera la liaison.
    </p>

    <FingerprintGroups :fingerprint="fingerprint" show-full />

    <p class="muted">
      Liaison fermée · aucune donnée d'authentification envoyée.
    </p>
    <div class="actions">
      <button type="button" :disabled="busy" @click="emit('refuse')">
        Refuser
      </button>
      <button type="button" :disabled="busy" @click="emit('accept')">
        L'empreinte correspond, accepter
      </button>
    </div>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  padding: var(--sp-4);
  border: var(--border-width) solid var(--color-border);
  border-radius: var(--r-2);
}

.kicker,
.muted {
  margin: 0;
  color: var(--color-text-muted);
  font-size: var(--fs-md);
}

h2 {
  margin: 0;
  font-size: var(--fs-xl);
}

.command {
  margin: 0;
  padding: var(--sp-2) var(--sp-3);
  background: var(--color-surface);
  font-family: var(--font-mono);
  font-size: var(--fs-md);
  white-space: pre-wrap;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--sp-3);
}
</style>
