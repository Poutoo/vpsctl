<script setup lang="ts">
/**
 * Champ de filtre d'une liste. Au repos, rappelle le raccourci Ctrl+F ;
 * rempli, propose de l'effacer. Ctrl+F y place le curseur (voir
 * `findShortcut`) ; Échap le vide, puis, une seconde fois, émet `leave`
 * pour rendre le focus à la liste.
 */
import { onBeforeUnmount, onMounted, ref } from "vue";
import { registerFindTarget } from "../../findShortcut";
import AppIcon from "./AppIcon.vue";

const props = withDefaults(
  defineProps<{
    /** Nom accessible et texte indicatif. */
    label: string;
    /** Faux quand Ctrl+F ne doit pas prendre le focus (saisie en cours). */
    shortcutEnabled?: boolean;
  }>(),
  { shortcutEnabled: true },
);

const emit = defineEmits<{ leave: [] }>();

const query = defineModel<string>({ required: true });
const input = ref<HTMLInputElement | null>(null);
const focused = ref(false);

function focus() {
  input.value?.focus();
  input.value?.select();
}

function clear() {
  query.value = "";
  input.value?.focus();
}

function onEscape(event: KeyboardEvent) {
  event.preventDefault();
  event.stopPropagation();
  if (query.value !== "") query.value = "";
  else emit("leave");
}

let unregister: (() => void) | null = null;
onMounted(() => {
  unregister = registerFindTarget({
    focus,
    enabled: () => props.shortcutEnabled,
  });
});
onBeforeUnmount(() => unregister?.());

defineExpose({ focus });
</script>

<template>
  <div class="filter" :class="{ focused, filled: query !== '' }">
    <AppIcon class="glyph" name="search" />
    <input
      ref="input"
      v-model="query"
      type="search"
      class="input"
      spellcheck="false"
      autocomplete="off"
      :placeholder="label"
      :aria-label="label"
      aria-keyshortcuts="Control+F"
      @focus="focused = true"
      @blur="focused = false"
      @keydown.esc="onEscape"
    />
    <button
      v-if="query !== ''"
      type="button"
      class="clear"
      aria-label="Effacer le filtre"
      @click="clear"
    >
      <AppIcon name="close" />
    </button>
    <span v-else-if="!focused" class="keys" aria-hidden="true">
      <kbd>Ctrl</kbd><kbd>F</kbd>
    </span>
  </div>
</template>

<style scoped>
.filter {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  width: var(--filter-width);
  height: var(--button-height-compact);
  padding: 0 var(--sp-1) 0 var(--sp-2);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-1);
  transition: border-color var(--dur-fast) var(--ease-out);
}

.filter.focused {
  border-color: var(--color-action);
}

/* Le contour du champ porte le focus : pas d'anneau sur la saisie. */
.input:focus-visible {
  outline: none;
}

@media (max-width: 1100px) {
  .filter {
    width: var(--filter-width-compact);
  }
}

.glyph {
  color: var(--color-text-muted);
}

.input {
  flex: 1;
  min-width: 0;
  padding: 0;
  background: transparent;
  border: none;
  color: var(--color-text);
  font-family: var(--font-mono);
  font-size: var(--fs-data);
}

.input::placeholder {
  color: var(--color-text-subtle);
}

/* Effacement natif des champs de recherche : remplacé par le bouton. */
.input::-webkit-search-cancel-button {
  appearance: none;
}

.keys {
  display: inline-flex;
  flex-shrink: 0;
  gap: var(--sp-1);
}

kbd {
  padding: 0 var(--sp-1);
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-1);
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
}

.clear {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--icon-button-size);
  height: var(--icon-button-size);
  padding: 0;
  background: none;
  border: none;
  border-radius: var(--r-1);
  color: var(--color-text-muted);
  cursor: pointer;
}

.clear:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}
</style>
