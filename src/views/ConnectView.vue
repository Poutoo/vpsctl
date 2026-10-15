<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { errorMessage, toAppError } from "../api/errors";
import ConnectRadar from "../components/features/ConnectRadar.vue";
import ConnectSteps from "../components/features/ConnectSteps.vue";
import HostKeyMismatchAlert from "../components/features/HostKeyMismatchAlert.vue";
import HostKeyUnknownPanel from "../components/features/HostKeyUnknownPanel.vue";
import PassphrasePrompt from "../components/features/PassphrasePrompt.vue";
import PageHeader from "../components/layout/PageHeader.vue";
import AppButton from "../components/ui/AppButton.vue";
import StateMessage from "../components/ui/StateMessage.vue";
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

/** État de la liaison : code de télémétrie et message, selon la phase. */
const signal = computed(() => {
  const profile = attemptProfile.value;
  const name = profile?.name ?? "";
  switch (connection.phase) {
    case "connecting":
      return {
        tone: "acquisition",
        code: "ACQUISITION",
        message: `Recherche de ${name}…`,
      };
    case "connected": {
      const target = profile ? ` · ${profile.user}@${profile.host}` : "";
      return {
        tone: "aos",
        code: "AOS",
        message: `Signal acquis · ${name}${target} · ouverture du tableau de bord…`,
      };
    }
    case "failed":
      return {
        tone: "los",
        code: "LOS",
        message: connection.error ? errorMessage(connection.error) : "",
      };
    default:
      return {
        tone: "idle",
        code: "EN ATTENTE",
        message: "Sélectionnez un profil puis établissez la liaison.",
      };
  }
});

/** Début de la tentative en cours, pour le chrono de la télémétrie. */
const attemptStartedAt = ref<number | null>(null);
watch(
  () => connection.phase,
  (phase) => {
    if (phase === "connecting") attemptStartedAt.value = performance.now();
  },
  { immediate: true },
);

/** Plot du serveur sur le radar, une fois le signal acquis. */
const radarTarget = computed(() =>
  connection.phase === "connected" && attemptProfile.value
    ? {
        name: attemptProfile.value.name,
        latencyMs: connection.info?.latencyMs ?? null,
      }
    : null,
);

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
      <PageHeader
        kicker="Acquisition du signal"
        title="Liaison au serveur"
        title-id="connect-title"
      />
      <p class="intro">Choisissez un profil pour établir la liaison.</p>

      <StateMessage v-if="loadError" tone="error" kicker="Profils illisibles">
        {{ loadError }}
      </StateMessage>

      <StateMessage
        v-if="profiles.loaded && profiles.profiles.length === 0"
        kicker="0 profil"
        title="Aucun profil pour l'instant"
      >
        Créez un profil pour établir une première liaison.
      </StateMessage>
      <fieldset v-else class="list" :disabled="connecting">
        <legend class="caps legend">
          <span>Profils</span>
          <span>{{ profiles.profiles.length }}</span>
        </legend>
        <label
          v-for="p in profiles.profiles"
          :key="p.id"
          class="profile"
          :class="{ selected: p.id === selectedId }"
        >
          <input
            v-model="selectedId"
            type="radio"
            name="profile"
            :value="p.id"
          />
          <span class="profile-main">
            <span class="name">{{ p.name }}</span>
            <span class="endpoint" :title="`${p.user}@${p.host}:${p.port}`">
              {{ p.user }}@{{ p.host }}:{{ p.port }}
            </span>
          </span>
          <span class="tags">
            <span v-if="p.isDefault" class="caps tag">par défaut</span>
            <span v-if="!p.hostKeyFingerprint" class="caps tag unknown">
              clé inconnue
            </span>
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
        <AppButton v-if="connecting" @click="connection.cancel()">
          Interrompre
        </AppButton>
        <AppButton
          v-else-if="connection.phase !== 'connected'"
          variant="primary"
          :disabled="!selectedId || busy"
          @click="connect"
        >
          Établir la liaison
        </AppButton>
        <AppButton :to="{ name: 'settings' }" :disabled="connecting">
          Nouveau profil
        </AppButton>
      </div>
    </section>

    <section class="scope" aria-label="État de la liaison">
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
        v-else-if="connection.phase === 'passphrase'"
        :key-path="attemptProfile?.keyPath ?? ''"
        :endpoint="endpoint"
        :rejected="connection.passphraseRejected"
        :busy="busy"
        @submit="submitPassphrase"
        @cancel="connection.reset()"
      />

      <div v-else class="radar-zone">
        <ConnectRadar :sweeping="connecting" :target="radarTarget" />
        <div class="state" :class="signal.tone" aria-live="polite">
          <p class="caps state-kicker">État de la liaison</p>
          <p class="state-code">{{ signal.code }}</p>
          <p
            class="state-message"
            :role="connection.phase === 'failed' ? 'alert' : undefined"
          >
            {{ signal.message }}
          </p>
        </div>
      </div>

      <ConnectSteps
        class="telemetry"
        :steps="connection.steps"
        :phase="connection.phase"
        :endpoint="endpoint"
        :profile-name="attemptProfile?.name ?? null"
        :started-at="attemptStartedAt"
      />
    </section>
  </div>
</template>

<style scoped>
.layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  height: 100%;
}

/* Colonne des profils : surface opaque (texte dense). */
.profiles {
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
  min-height: 0;
  overflow: auto;
  padding: var(--sp-6) var(--sp-5) var(--sp-5);
  background: var(--color-surface);
  border-right: var(--border-width) solid var(--color-border);
}

.intro {
  color: var(--color-text-muted);
  font-size: var(--fs-md);
}

.list {
  min-width: 0;
  margin: 0;
  padding: 0;
  border: none;
}

.legend {
  display: flex;
  float: left;
  justify-content: space-between;
  width: 100%;
  padding: 0 0 var(--sp-2);
  color: var(--color-text-muted);
}

.profile {
  display: flex;
  clear: both;
  align-items: flex-start;
  gap: var(--sp-3);
  padding: var(--sp-3);
  border-bottom: var(--border-width) solid var(--color-border);
  cursor: pointer;
  transition: background-color var(--dur-fast) var(--ease-out);
}

.profile:first-of-type {
  border-top: var(--border-width) solid var(--color-border);
}

.profile:hover {
  background: var(--color-surface-hover);
}

.profile.selected {
  background: var(--color-selection);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-action);
}

.list:disabled .profile {
  cursor: default;
}

.list:disabled .profile:not(.selected),
.list:disabled .profile:not(.selected) .endpoint {
  color: var(--color-text-subtle);
}

.profile input {
  margin: var(--sp-1) 0 0;
}

.profile-main {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.name {
  font-size: var(--fs-md);
  font-weight: 600;
}

.endpoint {
  overflow: hidden;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tags {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: var(--sp-1);
}

.tag {
  color: var(--color-action);
}

.tag.unknown {
  color: var(--color-text-muted);
}

.details {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: var(--sp-1) var(--sp-5);
  margin: 0;
  font-size: var(--fs-sm);
}

.details dt {
  color: var(--color-text-muted);
}

.details dd {
  margin: 0;
  overflow: hidden;
  font-family: var(--font-mono);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-3);
  margin-top: auto;
  padding-top: var(--sp-4);
  border-top: var(--border-width) solid var(--color-border);
}

/* Colonne de la liaison : l'image de fond reste visible autour du radar. */
.scope {
  display: flex;
  flex-direction: column;
  gap: var(--sp-5);
  min-height: 0;
  overflow: auto;
  padding: var(--sp-6);
}

.radar-zone {
  display: flex;
  flex: 1 1 0;
  gap: var(--sp-6);
  min-height: calc(4 * var(--sp-8));
}

.state {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--sp-2);
  min-width: 0;
  padding-top: var(--sp-5);
}

.state-kicker {
  color: var(--color-text-muted);
}

.state-code {
  font-family: var(--font-mono);
  font-size: var(--fs-3xl);
  font-weight: 500;
  letter-spacing: var(--tracking-caps);
  line-height: var(--lh-title);
}

.idle .state-code {
  color: var(--color-text-muted);
}

.acquisition .state-code {
  color: var(--color-warning);
}

.aos .state-code {
  color: var(--color-success);
}

.los .state-code {
  color: var(--color-danger-text);
}

.state-message {
  max-width: 40ch;
  font-size: var(--fs-md);
  font-weight: 600;
}

.telemetry {
  flex: 0 1 calc(6 * var(--row-height));
  min-height: calc(4 * var(--row-height));
}
</style>
