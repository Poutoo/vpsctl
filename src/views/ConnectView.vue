<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { errorMessage, toAppError } from "../api/errors";
import ConnectSteps from "../components/features/ConnectSteps.vue";
import HostKeyMismatchAlert from "../components/features/HostKeyMismatchAlert.vue";
import HostKeyUnknownPanel from "../components/features/HostKeyUnknownPanel.vue";
import PassphrasePrompt from "../components/features/PassphrasePrompt.vue";
import { useConnectionStore } from "../stores/connection";
import { useProfilesStore } from "../stores/profiles";

const router = useRouter();
const connection = useConnectionStore();
const profiles = useProfilesStore();

const selectedId = ref<string | null>(null);
const busy = ref(false);
const loadError = ref<string | null>(null);

const selected = computed(() => profiles.byId(selectedId.value));
/** Profil de la tentative en cours (peut différer de la sélection). */
const attemptProfile = computed(
  () => profiles.byId(connection.profileId) ?? selected.value,
);
const endpoint = computed(() =>
  attemptProfile.value
    ? `${attemptProfile.value.host}:${attemptProfile.value.port}`
    : "",
);
const connecting = computed(() => connection.phase === "connecting");

const stateTitle = computed(() => {
  switch (connection.phase) {
    case "connecting":
      return "Connexion en cours…";
    case "connected":
      return "Liaison établie";
    case "failed":
      return "Échec de la liaison";
    case "hostKeyUnknown":
      return "Empreinte à vérifier";
    case "passphrase":
      return "Passphrase requise";
    default:
      return "En attente";
  }
});

onMounted(async () => {
  try {
    await profiles.ensureLoaded();
  } catch (error) {
    loadError.value = errorMessage(toAppError(error));
  }
  selectedId.value = connection.profileId ?? profiles.preferred?.id ?? null;
});

// Après la connexion : redirection automatique vers le tableau de bord.
watch(
  () => connection.phase,
  (phase) => {
    if (phase === "connected") void router.push({ name: "dashboard" });
  },
  { immediate: true },
);

function connect() {
  if (selectedId.value) void connection.connect(selectedId.value);
}

async function withBusy(action: () => Promise<void>) {
  busy.value = true;
  try {
    await action();
  } finally {
    busy.value = false;
  }
}

function submitPassphrase(passphrase: string, remember: boolean) {
  void withBusy(() => connection.submitPassphrase(passphrase, remember));
}

function trustAndReconnect() {
  void withBusy(() => connection.trustAndReconnect());
}

function fingerprintPreview(fingerprint: string | null): string {
  return fingerprint ? `${fingerprint.slice(0, 18)}…` : "jamais vérifiée";
}
</script>

<template>
  <HostKeyMismatchAlert
    v-if="
      connection.phase === 'hostKeyMismatch' &&
      connection.expectedFingerprint &&
      connection.presentedFingerprint
    "
    :profile-name="attemptProfile?.name ?? ''"
    :endpoint="endpoint"
    :expected="connection.expectedFingerprint"
    :received="connection.presentedFingerprint"
    :busy="busy"
    @back="connection.reset()"
    @replace="trustAndReconnect"
  />

  <div v-else class="layout">
    <section class="profiles" aria-labelledby="connect-title">
      <h1 id="connect-title">Liaison au serveur</h1>
      <p class="muted">Choisissez un profil pour établir la liaison.</p>

      <p v-if="loadError" class="error">{{ loadError }}</p>

      <p v-if="profiles.loaded && profiles.profiles.length === 0">
        Aucun profil pour l'instant.
      </p>
      <fieldset v-else class="list" :disabled="connecting">
        <legend>Profils · {{ profiles.profiles.length }}</legend>
        <label v-for="p in profiles.profiles" :key="p.id" class="profile">
          <input
            v-model="selectedId"
            type="radio"
            name="profile"
            :value="p.id"
          />
          <span>
            <strong>{{ p.name }}</strong>
            <span v-if="p.isDefault" class="muted"> · par défaut</span>
            <br />
            <span class="mono">{{ p.user }}@{{ p.host }}:{{ p.port }}</span>
          </span>
        </label>
      </fieldset>

      <dl v-if="selected" class="details">
        <dt>Utilisateur</dt>
        <dd>{{ selected.user }}</dd>
        <dt>Hôte</dt>
        <dd>{{ selected.host }}:{{ selected.port }}</dd>
        <dt>Clé privée</dt>
        <dd>{{ selected.keyPath }}</dd>
        <dt>Empreinte</dt>
        <dd>{{ fingerprintPreview(selected.hostKeyFingerprint) }}</dd>
      </dl>

      <div class="actions">
        <button v-if="connecting" type="button" @click="connection.cancel()">
          Interrompre
        </button>
        <button
          v-else
          type="button"
          :disabled="!selectedId || busy"
          @click="connect"
        >
          Établir la liaison
        </button>
        <RouterLink :to="{ name: 'settings' }">Nouveau profil</RouterLink>
      </div>
    </section>

    <section class="state" aria-live="polite">
      <p class="muted">État de la liaison</p>
      <h2>{{ stateTitle }}</h2>
      <p
        v-if="connection.phase === 'failed' && connection.error"
        class="error"
        role="alert"
      >
        {{ errorMessage(connection.error) }}
      </p>

      <HostKeyUnknownPanel
        v-if="
          connection.phase === 'hostKeyUnknown' &&
          connection.presentedFingerprint
        "
        :fingerprint="connection.presentedFingerprint"
        :endpoint="endpoint"
        :busy="busy"
        @accept="trustAndReconnect"
        @refuse="connection.reset()"
      />

      <PassphrasePrompt
        v-if="connection.phase === 'passphrase'"
        :key-path="attemptProfile?.keyPath ?? ''"
        :endpoint="endpoint"
        :rejected="connection.passphraseRejected"
        :busy="busy"
        @submit="submitPassphrase"
        @cancel="connection.reset()"
      />

      <h3 class="telemetry-title">
        Télémétrie de liaison
        <span v-if="attemptProfile" class="muted">
          · {{ attemptProfile.name }}</span
        >
      </h3>
      <ConnectSteps
        :steps="connection.steps"
        :phase="connection.phase"
        :endpoint="endpoint"
      />
    </section>
  </div>
</template>

<style scoped>
.layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  gap: var(--sp-6);
}

.profiles,
.state {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
}

.muted {
  margin: 0;
  color: var(--color-text-muted);
  font-size: var(--fs-md);
}

.error {
  margin: 0;
  color: var(--color-danger-text);
}

h2 {
  margin: 0;
  font-size: var(--fs-2xl);
}

.list {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  margin: 0;
  padding: var(--sp-3);
  border: var(--border-width) solid var(--color-border);
  border-radius: var(--r-2);
}

.profile {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-2);
}

.mono {
  font-family: var(--font-mono);
  font-size: var(--fs-md);
}

.details {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: var(--sp-1) var(--sp-3);
  margin: 0;
  font-size: var(--fs-md);
}

.details dt {
  color: var(--color-text-muted);
}

.details dd {
  margin: 0;
  font-family: var(--font-mono);
  word-break: break-all;
}

.actions {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
}

.telemetry-title {
  margin: var(--sp-4) 0 0;
  font-size: var(--fs-lg);
}
</style>
