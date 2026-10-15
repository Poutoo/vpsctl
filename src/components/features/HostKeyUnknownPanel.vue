<script setup lang="ts">
import { computed } from "vue";
import AppButton from "../ui/AppButton.vue";
import PanelCard from "../ui/PanelCard.vue";
import StatusIcon from "../ui/StatusIcon.vue";
import FingerprintGroups from "./FingerprintGroups.vue";

const props = defineProps<{
  fingerprint: string;
  endpoint: string;
  busy: boolean;
}>();

const emit = defineEmits<{
  accept: [];
  refuse: [];
}>();

const groupCount = computed(() =>
  Math.ceil(props.fingerprint.replace(/^SHA256:/, "").length / 4),
);
</script>

<template>
  <PanelCard tone="warning" aria-labelledby="unknown-title">
    <template #kicker>
      <StatusIcon name="half" />
      Première acquisition · clé inconnue
    </template>
    <template #aside>{{ endpoint }}</template>

    <div class="columns">
      <div class="explain">
        <h2 id="unknown-title">Comparez l'empreinte du serveur</h2>
        <p>
          C'est la première liaison avec cet hôte. Vérifiez cette empreinte par
          un autre canal, par exemple la console de votre hébergeur :
        </p>
        <pre class="command">
ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub</pre>
        <p class="muted">
          Une fois acceptée, l'empreinte est mémorisée dans le profil et une
          nouvelle liaison démarre. Toute différence future bloquera la liaison.
        </p>
      </div>

      <div class="print">
        <p class="caps print-head">
          <span>SHA256</span>
          <span>{{ groupCount }} groupes</span>
        </p>
        <div class="print-box">
          <FingerprintGroups :fingerprint="fingerprint" show-full />
        </div>
      </div>
    </div>

    <template #footer>
      <p class="caps note">
        Liaison fermée · aucune donnée d'authentification envoyée
      </p>
      <AppButton :disabled="busy" @click="emit('refuse')">Refuser</AppButton>
      <AppButton variant="primary" :disabled="busy" @click="emit('accept')">
        L'empreinte correspond, accepter
      </AppButton>
    </template>
  </PanelCard>
</template>

<style scoped>
.columns {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-5);
}

.explain {
  display: flex;
  flex: 1 1 calc(3 * var(--sp-8));
  flex-direction: column;
  gap: var(--sp-3);
  font-size: var(--fs-md);
}

.muted {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.command {
  margin: 0;
  padding: var(--sp-2) var(--sp-3);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  font-weight: 500;
  white-space: pre-wrap;
  word-break: break-all;
}

/* Largeur de la grille : la chaîne complète passe à la ligne dessous. */
.print {
  display: flex;
  flex: 0 0 min-content;
  flex-direction: column;
  gap: var(--sp-2);
}

.print-head {
  display: flex;
  justify-content: space-between;
  gap: var(--sp-4);
  color: var(--color-text-muted);
}

.print-box {
  padding: var(--sp-4);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border);
}

.note {
  flex: 1;
  color: var(--color-text-muted);
}
</style>
