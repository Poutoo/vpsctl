<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { errorMessage, toAppError } from "../api/errors";
import type { Profile } from "../bindings/Profile";
import ProfileForm from "../components/features/ProfileForm.vue";
import { useConfirm } from "../composables/useConfirm";
import { useConnectionStore } from "../stores/connection";
import { useProfilesStore } from "../stores/profiles";

const profiles = useProfilesStore();
const connection = useConnectionStore();
const { confirm } = useConfirm();

/** Identifiant édité ; chaîne vide pour un nouveau profil. */
const editedId = ref<string | null>(null);
const busy = ref(false);
const error = ref<string | null>(null);

function blankProfile(): Profile {
  return {
    id: "",
    name: "",
    host: "",
    port: 22,
    user: "",
    keyPath: "~/.ssh/id_ed25519",
    hostKeyFingerprint: null,
    isDefault: profiles.profiles.length === 0,
  };
}

const edited = computed<Profile | null>(() => {
  if (editedId.value === "") return blankProfile();
  return profiles.byId(editedId.value);
});

const linkedId = computed(() =>
  connection.isConnected ? connection.profileId : null,
);

onMounted(async () => {
  try {
    await profiles.load();
  } catch (e) {
    error.value = errorMessage(toAppError(e));
  }
  editedId.value = profiles.preferred?.id ?? "";
});

function select(id: string) {
  error.value = null;
  editedId.value = id;
}

async function run(action: () => Promise<void>) {
  busy.value = true;
  error.value = null;
  try {
    await action();
  } catch (e) {
    error.value = errorMessage(toAppError(e));
  } finally {
    busy.value = false;
  }
}

function save(profile: Profile) {
  void run(async () => {
    const saved = await profiles.save(profile);
    editedId.value = saved.id;
  });
}

async function remove() {
  const profile = edited.value;
  if (!profile || profile.id === "") return;
  const confirmed = await confirm({
    title: `Supprimer le profil « ${profile.name} » ?`,
    message:
      "Le profil, son empreinte mémorisée et sa passphrase dans le trousseau seront supprimés. Cette action est définitive.",
    confirmLabel: "Supprimer le profil",
    danger: true,
  });
  if (!confirmed) return;
  void run(async () => {
    await profiles.remove(profile.id);
    editedId.value = profiles.preferred?.id ?? "";
  });
}

function makeDefault() {
  const id = edited.value?.id;
  if (id) void run(() => profiles.setDefault(id));
}
</script>

<template>
  <div class="settings">
    <header class="header">
      <h1>Réglages · profils de liaison</h1>
      <RouterLink v-if="!connection.isConnected" :to="{ name: 'connect' }">
        Retour à la connexion
      </RouterLink>
      <button type="button" :disabled="busy" @click="select('')">
        Nouveau profil
      </button>
    </header>

    <div class="layout">
      <ul class="list">
        <li v-for="p in profiles.profiles" :key="p.id">
          <button
            type="button"
            class="item"
            :aria-current="p.id === editedId"
            @click="select(p.id)"
          >
            <strong>{{ p.name }}</strong>
            <span v-if="p.isDefault" class="muted"> · par défaut</span>
            <span v-if="p.id === linkedId" class="muted">
              · liaison en cours</span
            >
            <br />
            <span class="mono">{{ p.user }}@{{ p.host }}:{{ p.port }}</span>
          </button>
        </li>
        <li v-if="editedId === ''" class="muted">Nouveau profil…</li>
      </ul>

      <ProfileForm
        v-if="edited"
        :key="edited.id || 'nouveau'"
        :profile="edited"
        :linked="edited.id !== '' && edited.id === linkedId"
        :busy="busy"
        :error="error"
        @save="save"
        @remove="remove"
        @make-default="makeDefault"
      />
    </div>
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.header {
  display: flex;
  align-items: center;
  gap: var(--space-4);
}

.header h1 {
  margin: 0;
  margin-right: auto;
}

.layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  gap: var(--space-5);
  align-items: start;
}

.list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.item {
  width: 100%;
  padding: var(--space-2) var(--space-3);
  text-align: left;
}

.item[aria-current="true"] {
  border-color: var(--color-accent);
}

.muted {
  color: var(--color-text-muted);
  font-size: var(--font-size-sm);
}

.mono {
  font-family: var(--font-mono);
  font-size: var(--font-size-sm);
}
</style>
