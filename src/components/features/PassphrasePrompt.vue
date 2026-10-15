<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import AppButton from "../ui/AppButton.vue";
import PanelCard from "../ui/PanelCard.vue";
import StatusIcon from "../ui/StatusIcon.vue";

const props = defineProps<{
  keyPath: string;
  endpoint: string;
  /** La dernière passphrase saisie était incorrecte. */
  rejected: boolean;
  busy: boolean;
}>();

const emit = defineEmits<{
  submit: [passphrase: string, remember: boolean];
  cancel: [];
}>();

const passphrase = ref("");
const remember = ref(true);
const input = ref<HTMLInputElement | null>(null);

onMounted(() => input.value?.focus());

watch(
  () => props.rejected,
  async (rejected) => {
    if (!rejected) return;
    await nextTick();
    input.value?.select();
  },
);

function submit() {
  if (!passphrase.value || props.busy) return;
  emit("submit", passphrase.value, remember.value);
  // La passphrase ne reste pas dans le composant après l'envoi.
  passphrase.value = "";
}
</script>

<template>
  <PanelCard
    tag="form"
    tone="warning"
    aria-labelledby="passphrase-title"
    @submit.prevent="submit"
  >
    <template #kicker>
      <StatusIcon name="half" />
      Passphrase requise · clé chiffrée
    </template>
    <template #aside>{{ endpoint }}</template>

    <div class="columns">
      <div class="form">
        <h2 id="passphrase-title">Déverrouiller la clé privée</h2>
        <p class="path">{{ keyPath }}</p>

        <label class="field">
          <span class="label">Passphrase</span>
          <input
            ref="input"
            v-model="passphrase"
            type="password"
            autocomplete="off"
            spellcheck="false"
            :aria-invalid="rejected"
            aria-describedby="passphrase-error"
          />
        </label>
        <p v-if="rejected" id="passphrase-error" class="error" role="alert">
          <StatusIcon class="shape" name="triangle" />
          Passphrase incorrecte. Réessayez.
        </p>

        <label class="check">
          <input v-model="remember" type="checkbox" />
          Mémoriser dans le trousseau du système
        </label>
      </div>

      <aside class="local">
        <p class="caps local-title">Traitement local</p>
        <p>
          La passphrase déverrouille la clé sur cet ordinateur. Elle n'est
          jamais envoyée au serveur.
        </p>
        <p>
          Mémorisée dans le trousseau, elle sera utilisée automatiquement aux
          prochaines liaisons.
        </p>
      </aside>
    </div>

    <template #footer>
      <p class="caps note">Entrée = déverrouiller</p>
      <AppButton @click="emit('cancel')">Annuler</AppButton>
      <AppButton
        variant="primary"
        type="submit"
        :disabled="!passphrase || busy"
      >
        Déverrouiller
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

.form {
  display: flex;
  flex: 1 1 calc(3 * var(--sp-8));
  flex-direction: column;
  gap: var(--sp-3);
}

.path {
  color: var(--color-action);
  font-family: var(--font-mono);
  font-size: var(--fs-md);
  word-break: break-all;
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  margin-top: var(--sp-2);
}

.label {
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

.check {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  font-size: var(--fs-md);
  font-weight: 500;
  cursor: pointer;
}

.local {
  display: flex;
  flex: 1 1 calc(3 * var(--sp-8));
  flex-direction: column;
  gap: var(--sp-3);
  padding-left: var(--sp-5);
  border-left: var(--border-width) solid var(--color-border);
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.local-title {
  color: var(--color-text-muted);
}

.note {
  flex: 1;
  color: var(--color-text-muted);
}
</style>
