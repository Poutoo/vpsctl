<script setup lang="ts">
import { computed, ref } from "vue";
import FingerprintGroups from "./FingerprintGroups.vue";

const props = defineProps<{
  profileName: string;
  endpoint: string;
  expected: string;
  received: string;
  busy: boolean;
}>();

const emit = defineEmits<{
  back: [];
  replace: [];
}>();

const replacing = ref(false);
const verified = ref(false);

const identicalGroups = computed(() => {
  const groups = (fp: string) =>
    fp.replace(/^SHA256:/, "").match(/.{1,4}/g) ?? [];
  const expected = groups(props.expected);
  return groups(props.received).filter((g, i) => g === expected[i]).length;
});
</script>

<template>
  <section class="alert" role="alert" aria-labelledby="mismatch-title">
    <p class="banner">
      Alerte de sécurité · liaison interrompue · {{ profileName }} ·
      {{ endpoint }}
    </p>

    <h1 id="mismatch-title">L'empreinte du serveur a changé.</h1>
    <p>
      La clé présentée par l'hôte ne correspond pas à celle mémorisée dans le
      profil. Il peut s'agir d'une réinstallation du serveur, ou d'une
      interception. La liaison a été coupée avant l'authentification : aucun
      identifiant n'a été envoyé.
    </p>

    <div class="compare">
      <h2>Mémorisée</h2>
      <FingerprintGroups :fingerprint="expected" show-full />
      <h2>Reçue</h2>
      <FingerprintGroups
        :fingerprint="received"
        :compare-to="expected"
        show-full
      />
      <p class="muted">
        SHA256 · groupes différents soulignés · {{ identicalGroups }} groupe(s)
        identique(s) sur 11
      </p>
    </div>

    <ol class="procedure">
      <li>
        Ouvrez la console de l'hébergeur et lisez l'empreinte :
        <code>ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub</code>
      </li>
      <li>
        Elle correspond à la clé reçue et le serveur a été réinstallé :
        remplacez la clé mémorisée.
      </li>
      <li>
        Elle ne correspond pas : n'établissez pas la liaison depuis ce réseau et
        vérifiez-le.
      </li>
    </ol>

    <div class="actions">
      <button type="button" class="primary" @click="emit('back')">
        Revenir aux profils
      </button>
      <button
        v-if="!replacing"
        type="button"
        class="danger"
        @click="replacing = true"
      >
        Remplacer la clé mémorisée…
      </button>
    </div>

    <div v-if="replacing" class="replace">
      <label>
        <input v-model="verified" type="checkbox" />
        J'ai vérifié l'empreinte sur la console de l'hébergeur
      </label>
      <button
        type="button"
        class="danger"
        :disabled="!verified || busy"
        @click="emit('replace')"
      >
        Remplacer par la clé reçue et se connecter
      </button>
    </div>
  </section>
</template>

<style scoped>
.alert {
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
}

.banner {
  margin: 0;
  padding: var(--sp-2) var(--sp-3);
  border: var(--border-width) solid var(--color-danger);
  color: var(--color-danger-text);
  font-size: var(--fs-md);
  text-transform: uppercase;
}

h1 {
  margin: 0;
}

h2 {
  margin: var(--sp-3) 0 var(--sp-2);
  color: var(--color-text-muted);
  font-size: var(--fs-md);
  text-transform: uppercase;
}

.compare {
  padding: var(--sp-4);
  border: var(--border-width) solid var(--color-border);
}

.muted {
  margin: var(--sp-3) 0 0;
  color: var(--color-text-muted);
  font-size: var(--fs-md);
}

.procedure {
  margin: 0;
  padding-left: var(--sp-5);
}

.procedure li + li {
  margin-top: var(--sp-2);
}

code {
  font-family: var(--font-mono);
  word-break: break-all;
}

.actions,
.replace {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-3);
}

.danger {
  color: var(--color-danger-text);
}
</style>
