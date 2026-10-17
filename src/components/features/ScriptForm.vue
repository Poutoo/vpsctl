<script setup lang="ts">
/**
 * Création ou modification d'un script : nom, description facultative,
 * commande (plusieurs lignes, monospace) et confirmation GO / NO GO
 * obligatoire. La validation complète est faite par le backend ; ici, les
 * champs requis seulement.
 */
import { computed, reactive, useId } from "vue";
import type { Script } from "../../bindings/Script";
import AppButton from "../ui/AppButton.vue";
import PanelCard from "../ui/PanelCard.vue";
import StatusIcon from "../ui/StatusIcon.vue";

/** Longueur maximale du nom (contrôlée aussi par le backend). */
const MAX_NAME = 64;

const props = defineProps<{
  /** Script édité ; `id` vide pour un nouveau script. */
  script: Script;
  busy: boolean;
  error: string | null;
}>();

const emit = defineEmits<{
  save: [script: Script];
  cancel: [];
}>();

const draft = reactive<Script>({ ...props.script });
const touched = reactive({ name: false, command: false });

const isNew = computed(() => props.script.id === "");

const problems = computed(() => ({
  name: !draft.name.trim()
    ? "Le nom du script est requis."
    : draft.name.trim().length > MAX_NAME
      ? `${MAX_NAME} caractères au plus.`
      : null,
  command: draft.command.trim() ? null : "La commande est requise.",
}));

const shown = computed(() => ({
  name: touched.name ? problems.value.name : null,
  command: touched.command ? problems.value.command : null,
}));

const valid = computed(() =>
  Object.values(problems.value).every((p) => p === null),
);

const uid = useId();
const errorId = (field: string) => `${uid}-${field}-error`;
const helpId = (field: string) => `${uid}-${field}-help`;

function describedBy(field: "name" | "command"): string {
  return [helpId(field), shown.value[field] ? errorId(field) : null]
    .filter(Boolean)
    .join(" ");
}

function submit() {
  Object.assign(touched, { name: true, command: true });
  if (!valid.value || props.busy) return;
  emit("save", { ...draft });
}
</script>

<template>
  <PanelCard tag="form" novalidate @submit.prevent="submit">
    <template #kicker>
      {{ isNew ? "Nouveau" : "Modifier" }}
      <template v-if="!isNew">
        · <span class="verbatim">{{ script.name }}</span>
      </template>
    </template>

    <label class="field">
      <span class="label">Nom</span>
      <input
        v-model="draft.name"
        type="text"
        spellcheck="false"
        autocomplete="off"
        :aria-invalid="shown.name !== null"
        :aria-describedby="describedBy('name')"
        @blur="touched.name = true"
      />
      <span :id="helpId('name')" class="help">Affiché dans la liste.</span>
      <span v-if="shown.name" :id="errorId('name')" class="error">
        <StatusIcon class="shape" name="triangle" />{{ shown.name }}
      </span>
    </label>

    <label class="field">
      <span class="label">Description (facultative)</span>
      <input
        v-model="draft.description"
        type="text"
        :aria-describedby="helpId('description')"
      />
      <span :id="helpId('description')" class="help">
        Rappel affiché dans la liste.
      </span>
    </label>

    <label class="field">
      <span class="label">Commande</span>
      <textarea
        v-model="draft.command"
        class="command"
        rows="6"
        spellcheck="false"
        autocomplete="off"
        autocapitalize="off"
        wrap="off"
        :aria-invalid="shown.command !== null"
        :aria-describedby="describedBy('command')"
        @blur="touched.command = true"
      />
      <span :id="helpId('command')" class="help">
        Exécutée sur le serveur, éventuellement sur plusieurs lignes. Peut
        appeler un fichier du serveur, par exemple
        <span class="verbatim">~/scripts/deploy.sh</span>.
        <span class="subtle">
          Exécutée dans un shell de connexion :
          <span class="verbatim">~/.profile</span> est chargé, mais pas les
          réglages de <span class="verbatim">~/.bashrc</span> (nvm, alias…).
        </span>
      </span>
      <span v-if="shown.command" :id="errorId('command')" class="error">
        <StatusIcon class="shape" name="triangle" />{{ shown.command }}
      </span>
    </label>

    <label class="confirmation" :class="{ on: draft.requireConfirmation }">
      <input v-model="draft.requireConfirmation" type="checkbox" />
      <span class="confirmation-text">
        <span class="confirmation-title">
          Confirmation GO / NO GO obligatoire
        </span>
        <span class="help">
          Une fenêtre de confirmation s'affiche avant chaque exécution.
        </span>
      </span>
    </label>

    <p v-if="error" class="error save-error" role="alert">
      <StatusIcon class="shape" name="triangle" />{{ error }}
    </p>

    <template #footer>
      <span class="spacer" />
      <AppButton :disabled="busy" @click="emit('cancel')">Annuler</AppButton>
      <AppButton variant="primary" type="submit" :disabled="busy">
        Enregistrer
      </AppButton>
    </template>
  </PanelCard>
</template>

<style scoped>
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

/* Commande : monospace, sans retour à la ligne automatique, agrandissable. */
.command {
  min-height: calc(6 * var(--console-line-height) + 2 * var(--sp-2));
  line-height: var(--console-line-height);
  resize: vertical;
  white-space: pre;
}

.help {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.subtle {
  display: block;
  margin-top: var(--sp-1);
  color: var(--color-text-subtle);
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

/* Case GO / NO GO : bloc entier cliquable, bordé d'ambre quand elle est cochée. */
.confirmation {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-4);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border-strong);
  cursor: pointer;
  transition:
    background-color var(--dur-fast) var(--ease-out),
    border-color var(--dur-fast) var(--ease-out);
}

.confirmation.on {
  background: var(--color-selection);
  border-color: var(--color-action);
}

.confirmation input {
  margin-top: var(--sp-1);
}

.confirmation-text {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
}

.confirmation-title {
  font-family: var(--font-mono);
  font-size: var(--fs-md);
  font-weight: 600;
}

.spacer {
  flex: 1;
}
</style>
