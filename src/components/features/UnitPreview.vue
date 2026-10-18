<script setup lang="ts">
/**
 * Aperçu du fichier d'unité. Le texte vient toujours du backend
 * (`service_unit_preview`, la fonction qui produit aussi le fichier écrit),
 * demandé 300 ms après la dernière saisie, et seulement quand le formulaire
 * est valide.
 *
 * - Valide : texte du backend, étiquette LIVE.
 * - Invalide : dernier texte valide, estompé, avec le problème en cours ; ou,
 *   avant toute saisie valide, le gabarit avec « … » (illustration, pas le
 *   fichier).
 */
import { onBeforeUnmount, ref, watch } from "vue";
import { errorMessage, toAppError } from "../../api/errors";
import { previewUnit } from "../../api/systemd";
import type { ServiceDef } from "../../bindings/ServiceDef";
import PanelCard from "../ui/PanelCard.vue";
import StatusIcon from "../ui/StatusIcon.vue";

const props = defineProps<{
  def: ServiceDef;
  /** Premier problème de la saisie, ou `null` si elle est valide. */
  problem: string | null;
}>();

/** Délai après la dernière saisie avant de demander l'aperçu. */
const DEBOUNCE_MS = 300;

/** Gabarit affiché tant qu'aucune saisie n'a été valide. */
const SKELETON = `[Unit]
Description=…

[Service]
Type=simple
WorkingDirectory=…
ExecStart=…
Restart=…
RestartSec=5

[Install]
WantedBy=default.target`;

/** Dernier texte renvoyé par le backend. */
const text = ref<string | null>(null);
/** Le texte affiché correspond à la saisie en cours. */
const fresh = ref(false);
/** Refus du backend (règle non reproduite côté interface). */
const refused = ref<string | null>(null);

let timer: ReturnType<typeof setTimeout> | null = null;
/** Seule la réponse à la dernière demande est retenue. */
let request = 0;

watch(
  () => [props.def, props.problem] as const,
  ([def, problem]) => {
    if (timer) clearTimeout(timer);
    fresh.value = false;
    refused.value = null;
    const current = ++request;
    if (problem !== null) return;
    timer = setTimeout(async () => {
      try {
        const result = await previewUnit(def);
        if (current !== request) return;
        text.value = result;
        fresh.value = true;
      } catch (error) {
        if (current !== request) return;
        refused.value = errorMessage(toAppError(error));
      }
    }, DEBOUNCE_MS);
  },
  { immediate: true, deep: true },
);

onBeforeUnmount(() => {
  if (timer) clearTimeout(timer);
  request++;
});
</script>

<template>
  <PanelCard class="preview" aria-label="Aperçu du fichier d'unité">
    <template #kicker>
      Aperçu ·
      <span class="verbatim">{{ def.name || "nom" }}.service</span>
    </template>
    <template #aside>
      <span v-if="fresh" class="caps live">Live</span>
    </template>
    <pre
      class="unit"
      :class="{ stale: !fresh }"
      aria-live="off"
    ><code>{{ text ?? SKELETON }}</code></pre>
    <p v-if="problem ?? refused" class="pending" role="status">
      <StatusIcon class="shape" name="half" />
      <span>{{ problem ?? refused }}</span>
    </p>
  </PanelCard>
</template>

<style scoped>
.unit {
  margin: 0;
  overflow-x: auto;
  color: var(--color-text);
  font-family: var(--font-mono);
  font-size: var(--fs-data);
  line-height: var(--console-line-height);
  white-space: pre;
  transition: color var(--dur-fast) var(--ease-out);
}

/* Texte qui ne correspond plus à la saisie : en retrait, jamais masqué. */
.unit.stale {
  color: var(--color-text-muted);
}

.live {
  color: var(--color-action);
}

.pending {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-2);
  margin: 0;
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.shape {
  flex-shrink: 0;
  margin-top: var(--sp-1);
  color: var(--color-warning);
}
</style>
