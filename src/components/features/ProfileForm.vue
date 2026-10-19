<script setup lang="ts">
import { computed, reactive, useId, watch } from "vue";
import type { Profile } from "../../bindings/Profile";
import AppButton from "../ui/AppButton.vue";
import PanelCard from "../ui/PanelCard.vue";
import StatusIcon from "../ui/StatusIcon.vue";

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

/** Erreurs affichées : après le premier passage dans le champ (le port, toujours). */
const shown = computed(() => ({
  name: touched.name ? problems.value.name : null,
  host: touched.host ? problems.value.host : null,
  port: problems.value.port,
  user: touched.user ? problems.value.user : null,
  keyPath: touched.keyPath ? problems.value.keyPath : null,
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

/** Identifiants des messages d'erreur, pour `aria-describedby`. */
const uid = useId();
const errorId = (field: string) => `${uid}-${field}-error`;

function submit() {
  Object.assign(touched, { name: true, host: true, user: true, keyPath: true });
  if (!valid.value || props.busy) return;
  emit("save", { ...draft });
}
</script>

<template>
  <PanelCard tag="form" novalidate @submit.prevent="submit">
    <template #kicker>
      Profil ·
      <span class="verbatim">{{ isNew ? "nouveau" : profile.name }}</span>
    </template>
    <template #aside>
      <span class="caps" :class="{ 'is-default': profile.isDefault }">{{
        profile.isDefault ? "Par défaut" : "Secondaire"
      }}</span>
    </template>

    <label class="field">
      <span class="label">Nom du profil</span>
      <input
        v-model="draft.name"
        type="text"
        :aria-invalid="shown.name !== null"
        :aria-describedby="shown.name ? errorId('name') : undefined"
        @blur="touched.name = true"
      />
      <span v-if="shown.name" :id="errorId('name')" class="error">
        <StatusIcon class="shape" name="triangle" />{{ shown.name }}
      </span>
    </label>

    <div class="row">
      <label class="field">
        <span class="label">Hôte</span>
        <input
          v-model="draft.host"
          type="text"
          spellcheck="false"
          autocomplete="off"
          :aria-invalid="shown.host !== null"
          :aria-describedby="shown.host ? errorId('host') : undefined"
          @blur="touched.host = true"
        />
        <span v-if="shown.host" :id="errorId('host')" class="error">
          <StatusIcon class="shape" name="triangle" />{{ shown.host }}
        </span>
      </label>
      <label class="field">
        <span class="label">Port</span>
        <input
          v-model.number="draft.port"
          type="number"
          min="1"
          max="65535"
          :aria-invalid="shown.port !== null"
          :aria-describedby="shown.port ? errorId('port') : undefined"
        />
        <span v-if="shown.port" :id="errorId('port')" class="error">
          <StatusIcon class="shape" name="triangle" />{{ shown.port }}
        </span>
      </label>
    </div>

    <div class="row">
      <label class="field">
        <span class="label">Utilisateur</span>
        <input
          v-model="draft.user"
          type="text"
          spellcheck="false"
          autocomplete="off"
          :aria-invalid="shown.user !== null"
          :aria-describedby="shown.user ? errorId('user') : undefined"
          @blur="touched.user = true"
        />
        <span v-if="shown.user" :id="errorId('user')" class="error">
          <StatusIcon class="shape" name="triangle" />{{ shown.user }}
        </span>
      </label>
      <label class="field">
        <span class="label">Clé privée</span>
        <input
          v-model="draft.keyPath"
          type="text"
          spellcheck="false"
          autocomplete="off"
          placeholder="~/.ssh/id_ed25519"
          :aria-invalid="shown.keyPath !== null"
          :aria-describedby="shown.keyPath ? errorId('keyPath') : undefined"
          @blur="touched.keyPath = true"
        />
        <span v-if="shown.keyPath" :id="errorId('keyPath')" class="error">
          <StatusIcon class="shape" name="triangle" />{{ shown.keyPath }}
        </span>
      </label>
    </div>

    <p v-if="linked" class="notice" role="note">
      <StatusIcon class="notice-shape" name="triangle" />
      Liaison active : une modification d'hôte, de port ou d'utilisateur ne
      s'appliquera qu'après avoir rétabli la liaison.
    </p>

    <section class="fingerprint" aria-labelledby="fingerprint-title">
      <h3 id="fingerprint-title" class="label">
        Empreinte de la clé du serveur
      </h3>
      <template v-if="profile.hostKeyFingerprint">
        <p class="print">
          <StatusIcon class="print-shape" name="check" />
          <span class="print-text">{{ profile.hostKeyFingerprint }}</span>
        </p>
        <p class="help">
          Lecture seule. Elle ne change que par l'alerte de sécurité de l'écran
          de connexion.
        </p>
      </template>
      <p v-else class="print empty">
        Aucune empreinte mémorisée. Elle sera à vérifier à la première
        connexion.
      </p>
      <p v-if="endpointChanged" class="notice" role="status">
        <StatusIcon class="notice-shape" name="triangle" />
        L'hôte ou le port a changé : l'empreinte mémorisée sera effacée à
        l'enregistrement et devra être vérifiée à la prochaine connexion.
      </p>
    </section>

    <p v-if="error" class="error save-error" role="alert">
      <StatusIcon class="shape" name="triangle" />{{ error }}
    </p>

    <template #footer>
      <AppButton
        v-if="!isNew"
        variant="danger"
        :disabled="linked || busy"
        @click="emit('remove')"
      >
        Supprimer…
      </AppButton>
      <span v-if="!isNew && linked" class="hint">
        Profil connecté : coupez la liaison pour le supprimer.
      </span>
      <span class="spacer" />
      <AppButton
        v-if="!isNew && !profile.isDefault"
        :disabled="busy"
        @click="emit('makeDefault')"
      >
        Définir par défaut
      </AppButton>
      <AppButton variant="primary" type="submit" :disabled="busy">
        Enregistrer
      </AppButton>
    </template>
  </PanelCard>
</template>

<style scoped>
.is-default {
  color: var(--color-action);
}

.row {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--sp-4);
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.label {
  font-family: var(--font-ui);
  font-size: var(--fs-sm);
  font-weight: 500;
}

.error {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  color: var(--color-danger-text);
  font-size: var(--fs-sm);
}

.shape {
  color: var(--color-danger);
}

.notice {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-2);
  padding: var(--sp-2) var(--sp-3);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border-strong);
  font-size: var(--fs-sm);
}

.notice-shape {
  margin-top: var(--sp-1);
  color: var(--color-warning);
}

.fingerprint {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding-top: var(--sp-4);
  border-top: var(--border-width) solid var(--color-border);
}

.print {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  min-height: var(--row-height);
  padding: var(--sp-2) var(--sp-3);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border);
  border-radius: var(--r-1);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

.print-shape {
  color: var(--color-success);
}

.print-text {
  user-select: all;
  word-break: break-all;
}

.print.empty {
  border-style: dashed;
  color: var(--color-text-muted);
  font-family: var(--font-ui);
}

.help {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.hint {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.spacer {
  flex: 1;
}
</style>
