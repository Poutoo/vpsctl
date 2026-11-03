<script setup lang="ts">
/**
 * Zone d'édition : une zone de texte native (annuler, saisie avec accents,
 * sélection et accessibilité du moteur web), sans retour automatique à la
 * ligne, avec une gouttière de numéros synchronisée sur le défilement. Seuls
 * les numéros visibles sont rendus : un fichier de 1 Mo peut compter des
 * dizaines de milliers de lignes.
 *
 * Tab insère une tabulation ; Échap puis Tab sort de la zone, pour ne pas
 * piéger le clavier. Les insertions passent par `execCommand("insertText")`
 * pour rester dans l'historique d'annulation (Ctrl+Z).
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { lineIndexAt, replaceLines } from "../../editorText";

const props = defineProps<{
  modelValue: string;
  readonly?: boolean;
  /** Lignes modifiées depuis l'ouverture (index à partir de 0). */
  changed: ReadonlySet<number>;
  label: string;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
  cursor: [position: { line: number; column: number }];
  /** Échap pressé : le prochain Tab sortira de la zone. */
  escape: [armed: boolean];
}>();

const area = ref<HTMLTextAreaElement | null>(null);
const scrollTop = ref(0);
const viewport = ref(0);
const lineHeight = ref(20);
const cursorLine = ref(0);
let escapeArmed = false;

const lineCount = computed(() => {
  let count = 1;
  const text = props.modelValue;
  for (let i = text.indexOf("\n"); i >= 0; i = text.indexOf("\n", i + 1)) {
    count += 1;
  }
  return count;
});

const first = computed(() =>
  Math.max(0, Math.floor(scrollTop.value / lineHeight.value)),
);
const last = computed(() =>
  Math.min(
    lineCount.value,
    Math.ceil((scrollTop.value + viewport.value) / lineHeight.value) + 1,
  ),
);
const numbers = computed(() =>
  Array.from({ length: Math.max(0, last.value - first.value) }, (_, i) => {
    const index = first.value + i;
    return { index, changed: props.changed.has(index) };
  }),
);
const shift = computed(() => first.value * lineHeight.value - scrollTop.value);

function measure() {
  const element = area.value;
  if (!element) return;
  viewport.value = element.clientHeight;
  scrollTop.value = element.scrollTop;
  const height = Number.parseFloat(getComputedStyle(element).lineHeight);
  if (Number.isFinite(height) && height > 0) lineHeight.value = height;
}

function reportCursor() {
  const element = area.value;
  if (!element) return;
  const position = element.selectionStart;
  const text = element.value;
  const line = lineIndexAt(text, position);
  cursorLine.value = line;
  const lineStart = text.lastIndexOf("\n", position - 1) + 1;
  emit("cursor", { line: line + 1, column: position - lineStart + 1 });
}

function setEscape(armed: boolean) {
  if (escapeArmed === armed) return;
  escapeArmed = armed;
  emit("escape", armed);
}

/** Insère du texte à la place de la sélection, dans l'historique d'annulation. */
function insert(text: string) {
  const element = area.value;
  if (!element) return;
  if (!document.execCommand("insertText", false, text)) {
    element.setRangeText(
      text,
      element.selectionStart,
      element.selectionEnd,
      "end",
    );
    element.dispatchEvent(new Event("input", { bubbles: true }));
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    setEscape(true);
    return;
  }
  if (
    event.key === "Tab" &&
    !event.ctrlKey &&
    !event.altKey &&
    !event.metaKey
  ) {
    if (escapeArmed || event.shiftKey || props.readonly) {
      // Sortie de la zone : comportement normal du navigateur.
      setEscape(false);
      return;
    }
    event.preventDefault();
    insert("\t");
    return;
  }
  if (!["Shift", "Control", "Alt", "Meta"].includes(event.key)) {
    setEscape(false);
  }
}

function onInput() {
  emit("update:modelValue", area.value?.value ?? "");
  reportCursor();
}

function onScroll() {
  scrollTop.value = area.value?.scrollTop ?? 0;
}

function onSelection() {
  if (document.activeElement === area.value) reportCursor();
}

let observer: ResizeObserver | null = null;

onMounted(() => {
  measure();
  observer = new ResizeObserver(measure);
  if (area.value) observer.observe(area.value);
  document.addEventListener("selectionchange", onSelection);
});

onBeforeUnmount(() => {
  observer?.disconnect();
  document.removeEventListener("selectionchange", onSelection);
});

defineExpose({
  focus() {
    area.value?.focus();
  },
  /**
   * Remplace les lignes `[start, end)` par `lines`, en une seule opération
   * annulable par Ctrl+Z.
   */
  replace(start: number, end: number, lines: readonly string[]) {
    const element = area.value;
    if (!element || props.readonly) return;
    const change = replaceLines(element.value, start, end, lines);
    if (!change) return;
    element.focus();
    element.setSelectionRange(change.from, change.to);
    insert(change.insert);
  },
});
</script>

<template>
  <div class="code-area">
    <div class="gutter" aria-hidden="true">
      <div class="numbers" :style="{ transform: `translateY(${shift}px)` }">
        <span
          v-for="number in numbers"
          :key="number.index"
          class="number"
          :class="{
            changed: number.changed,
            current: number.index === cursorLine,
          }"
          >{{ number.index + 1 }}</span
        >
      </div>
    </div>
    <textarea
      ref="area"
      class="text"
      :value="modelValue"
      :readonly="readonly"
      :aria-label="label"
      wrap="off"
      spellcheck="false"
      autocapitalize="off"
      autocomplete="off"
      translate="no"
      data-testid="editor-text"
      @input="onInput"
      @keydown="onKeydown"
      @scroll="onScroll"
      @click="reportCursor"
      @blur="setEscape(false)"
    />
  </div>
</template>

<style scoped>
.code-area {
  display: flex;
  flex: 1;
  min-height: 0;
  background: var(--color-field);
}

.gutter {
  position: relative;
  flex-shrink: 0;
  width: var(--editor-gutter-width);
  overflow: hidden;
  border-right: var(--border-width) solid var(--color-border);
  font-family: var(--font-mono);
  font-size: var(--fs-data);
  line-height: var(--editor-line-height);
  color: var(--color-text-subtle);
  text-align: right;
  user-select: none;
}

.numbers {
  position: absolute;
  top: var(--sp-3);
  right: 0;
  left: 0;
}

.number {
  display: block;
  height: var(--editor-line-height);
  padding-right: var(--sp-3);
}

/* Ligne modifiée : trait d'action à gauche (forme, en plus de la couleur du numéro). */
.number.changed {
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-action);
}

.number.current {
  color: var(--color-text);
  font-weight: 600;
}

.text {
  flex: 1;
  min-width: 0;
  margin: 0;
  padding: var(--sp-3) var(--sp-4);
  overflow: auto;
  background: transparent;
  border: none;
  outline: none;
  color: var(--color-text);
  font-family: var(--font-mono);
  font-size: var(--fs-data);
  line-height: var(--editor-line-height);
  tab-size: var(--editor-tab-size);
  white-space: pre;
  resize: none;
}

.text:focus-visible {
  box-shadow: inset 0 0 0 var(--border-width) var(--color-action);
}

.text[readonly] {
  color: var(--color-text-muted);
}
</style>
