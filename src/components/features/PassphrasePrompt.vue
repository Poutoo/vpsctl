<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";

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
  <form
    class="panel"
    aria-labelledby="passphrase-title"
    @submit.prevent="submit"
  >
    <p class="kicker">Passphrase requise · clé chiffrée · {{ endpoint }}</p>
    <h2 id="passphrase-title">Déverrouiller la clé privée</h2>
    <p class="path">{{ keyPath }}</p>

    <label class="field">
      Passphrase
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
      Passphrase incorrecte. Réessayez.
    </p>

    <label class="check">
      <input v-model="remember" type="checkbox" />
      Mémoriser dans le trousseau du système
    </label>

    <p class="muted">
      La passphrase déverrouille la clé sur cet ordinateur. Elle n'est jamais
      envoyée au serveur. Mémorisée dans le trousseau, elle sera utilisée
      automatiquement aux prochaines liaisons.
    </p>

    <div class="actions">
      <button type="button" @click="emit('cancel')">Annuler</button>
      <button type="submit" :disabled="!passphrase || busy">
        Déverrouiller
      </button>
    </div>
  </form>
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

.path {
  margin: 0;
  font-family: var(--font-mono);
  font-size: var(--fs-md);
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
}

.check {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.error {
  margin: 0;
  color: var(--color-danger-text);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--sp-3);
}
</style>
