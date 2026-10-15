<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { errorMessage, toAppError } from "../api/errors";
import type { Profile } from "../bindings/Profile";
import ProfileForm from "../components/features/ProfileForm.vue";
import PageHeader from "../components/layout/PageHeader.vue";
import AppButton from "../components/ui/AppButton.vue";
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
    title: "Supprimer le profil ?",
    subject: profile.name,
    consequences: [
      "Le profil et son empreinte mémorisée seront supprimés.",
      "Sa passphrase sera retirée du trousseau du système.",
      "Cette action est définitive.",
    ],
    confirmLabel: "Supprimer",
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
    <PageHeader section="settings" title="Réglages">
      <AppButton v-if="!connection.isConnected" :to="{ name: 'connect' }">
        Retour à la connexion
      </AppButton>
      <AppButton variant="primary" :disabled="busy" @click="select('')">
        Nouveau profil
      </AppButton>
    </PageHeader>

    <div class="layout">
      <ul class="list" aria-label="Profils de liaison">
        <li v-for="p in profiles.profiles" :key="p.id">
          <button
            type="button"
            class="item"
            :aria-current="p.id === editedId"
            @click="select(p.id)"
          >
            <span class="item-head">
              <span class="name">{{ p.name }}</span>
              <span v-if="p.isDefault" class="caps tag">par défaut</span>
            </span>
            <span class="endpoint">{{ p.user }}@{{ p.host }}:{{ p.port }}</span>
            <span v-if="p.id === linkedId" class="caps linked">
              liaison en cours
            </span>
          </button>
        </li>
        <li v-if="editedId === ''">
          <span class="item new" aria-current="true">
            <span class="name">Nouveau profil…</span>
          </span>
        </li>
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
  gap: var(--sp-5);
}

.layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  gap: var(--sp-5);
  align-items: start;
}

/* Liste des profils : surface opaque, une ligne par profil. */
.list {
  margin: 0;
  padding: 0;
  list-style: none;
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.list li + li {
  border-top: var(--border-width) solid var(--color-border);
}

.item {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  width: 100%;
  padding: var(--sp-3) var(--sp-4);
  background: transparent;
  border: none;
  text-align: left;
  cursor: pointer;
  transition: background-color var(--dur-fast) var(--ease-out);
}

.item:hover {
  background: var(--color-surface-hover);
}

.item:focus-visible {
  outline-offset: calc(-1 * var(--focus-offset));
}

.item[aria-current="true"] {
  background: var(--color-surface-raised);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-action);
}

.item.new {
  cursor: default;
}

.item-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--sp-2);
}

.name {
  overflow: hidden;
  font-family: var(--font-title);
  font-size: var(--fs-lg);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tag {
  flex-shrink: 0;
  color: var(--color-action);
}

.endpoint {
  overflow: hidden;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.linked {
  color: var(--color-success);
}
</style>
