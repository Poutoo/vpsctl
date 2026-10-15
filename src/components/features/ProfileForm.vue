<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import type { Profile } from "../../bindings/Profile";

const props = defineProps<{
  /** Profil édité ; `id` vide pour un nouveau profil. */
  profile: Profile;
  /** Profil de la liaison en cours : suppression impossible. */
  linked: boolean;
  busy: boolean;
  error: string | null;
}>();

const emit = defineEmits<{
  save: [profile: Profile];
  remove: [];
  makeDefault: [];
}>();

const draft = reactive<Profile>({ ...props.profile });
const touched = reactive({
  name: false,
  host: false,
  user: false,
  keyPath: false,
});

watch(
  () => props.profile,
  (profile) => {
    Object.assign(draft, profile);
    Object.assign(touched, {
      name: false,
      host: false,
      user: false,
      keyPath: false,
    });
  },
);

const isNew = computed(() => props.profile.id === "");

const problems = computed(() => ({
  name: draft.name.trim() ? null : "Le nom du profil est requis.",
  host: draft.host.trim() ? null : "L'hôte est requis.",
  port:
    Number.isInteger(draft.port) && draft.port >= 1 && draft.port <= 65535
      ? null
      : "Le port doit être compris entre 1 et 65535.",
  user: draft.user.trim() ? null : "L'utilisateur est requis.",
  keyPath: draft.keyPath.trim()
    ? null
    : "Le chemin de la clé privée est requis.",
}));

const valid = computed(() =>
  Object.values(problems.value).every((p) => p === null),
);

/** L'empreinte mémorisée sera effacée par le backend à l'enregistrement. */
const endpointChanged = computed(
  () =>
    !isNew.value &&
    props.profile.hostKeyFingerprint !== null &&
    (draft.host.trim() !== props.profile.host ||
      draft.port !== props.profile.port),
);

function submit() {
  Object.assign(touched, { name: true, host: true, user: true, keyPath: true });
  if (!valid.value || props.busy) return;
  emit("save", { ...draft });
}
</script>

<template>
  <form class="form" novalidate @submit.prevent="submit">
    <p class="kicker">
      Profil · {{ isNew ? "nouveau" : profile.name }}
      <span v-if="profile.isDefault"> · par défaut</span>
    </p>

    <label class="field">
      Nom du profil
      <input v-model="draft.name" type="text" @blur="touched.name = true" />
      <span v-if="touched.name && problems.name" class="error">{{
        problems.name
      }}</span>
    </label>

    <div class="row">
      <label class="field grow">
        Hôte
        <input
          v-model="draft.host"
          type="text"
          spellcheck="false"
          autocomplete="off"
          @blur="touched.host = true"
        />
        <span v-if="touched.host && problems.host" class="error">{{
          problems.host
        }}</span>
      </label>
      <label class="field">
        Port
        <input v-model.number="draft.port" type="number" min="1" max="65535" />
        <span v-if="problems.port" class="error">{{ problems.port }}</span>
      </label>
    </div>

    <div class="row">
      <label class="field grow">
        Utilisateur
        <input
          v-model="draft.user"
          type="text"
          spellcheck="false"
          autocomplete="off"
          @blur="touched.user = true"
        />
        <span v-if="touched.user && problems.user" class="error">{{
          problems.user
        }}</span>
      </label>
      <label class="field grow">
        Clé privée
        <input
          v-model="draft.keyPath"
          type="text"
          spellcheck="false"
          autocomplete="off"
          placeholder="~/.ssh/id_ed25519"
          @blur="touched.keyPath = true"
        />
        <span v-if="touched.keyPath && problems.keyPath" class="error">{{
          problems.keyPath
        }}</span>
      </label>
    </div>

    <section class="fingerprint">
      <h3>Empreinte de la clé du serveur</h3>
      <p v-if="profile.hostKeyFingerprint" class="mono">
        {{ profile.hostKeyFingerprint }}
      </p>
      <p v-else class="muted">
        Aucune empreinte mémorisée. Elle sera à vérifier à la première
        connexion.
      </p>
      <p v-if="endpointChanged" class="warning" role="status">
        L'hôte ou le port a changé : l'empreinte mémorisée sera effacée à
        l'enregistrement et devra être vérifiée à la prochaine connexion.
      </p>
    </section>

    <p v-if="error" class="error" role="alert">{{ error }}</p>

    <div class="actions">
      <button
        v-if="!isNew"
        type="button"
        class="danger"
        :disabled="linked || busy"
        :title="
          linked
            ? 'Impossible de supprimer le profil de la liaison en cours'
            : undefined
        "
        @click="emit('remove')"
      >
        Supprimer…
      </button>
      <span class="spacer" />
      <button
        v-if="!isNew && !profile.isDefault"
        type="button"
        :disabled="busy"
        @click="emit('makeDefault')"
      >
        Définir par défaut
      </button>
      <button type="submit" :disabled="busy">Enregistrer</button>
    </div>
  </form>
</template>

<style scoped>
.form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-4);
  border: var(--border-width) solid var(--color-border);
  border-radius: var(--radius-md);
}

.kicker,
.muted {
  margin: 0;
  color: var(--color-text-muted);
  font-size: var(--font-size-sm);
}

.row {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-4);
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.grow {
  flex: 1;
}

.fingerprint h3 {
  margin: 0 0 var(--space-2);
  font-size: var(--font-size-md);
}

.fingerprint p {
  margin: 0;
}

.mono {
  font-family: var(--font-mono);
  word-break: break-all;
}

.warning {
  margin-top: var(--space-2);
  color: var(--color-warning);
}

.error {
  margin: 0;
  color: var(--color-danger);
  font-size: var(--font-size-sm);
}

.actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.spacer {
  flex: 1;
}

.danger {
  color: var(--color-danger);
}
</style>
