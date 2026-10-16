<script setup lang="ts">
/**
 * Itinéraire : fil d'Ariane du dossier affiché, de la racine (DÉPART, au nom
 * du serveur) jusqu'à la POSITION. Chaque étape parcourue est un bouton qui
 * y revient. Un long chemin défile horizontalement, la position restant
 * visible.
 */
import { computed, nextTick, ref, watch } from "vue";
import { pathFromSegments, pathSegments } from "../../remoteFiles";
import StatusIcon from "../ui/StatusIcon.vue";

const props = defineProps<{
  path: string;
  /** Nom du serveur, affiché à la racine. */
  server: string;
}>();

const emit = defineEmits<{ navigate: [path: string] }>();

interface Stop {
  path: string;
  label: string;
  root: boolean;
}

const stops = computed<Stop[]>(() => {
  const segments = pathSegments(props.path);
  return [
    { path: "/", label: props.server, root: true },
    ...segments.map((label, index) => ({
      path: pathFromSegments(segments.slice(0, index + 1)),
      label,
      root: false,
    })),
  ];
});

const track = ref<HTMLElement | null>(null);

watch(
  () => props.path,
  () =>
    void nextTick(() => {
      const element = track.value;
      if (element) element.scrollLeft = element.scrollWidth;
    }),
  { immediate: true },
);
</script>

<template>
  <nav class="itinerary" aria-label="Itinéraire">
    <ol ref="track" class="stops">
      <li v-for="(stop, index) in stops" :key="stop.path" class="stop">
        <span v-if="index > 0" class="leg" aria-hidden="true" />
        <span
          v-if="index === stops.length - 1"
          class="node current"
          aria-current="location"
        >
          <StatusIcon class="marker" name="disc" />
          <span class="label"
            >{{ stop.label }}<template v-if="stop.root"> /</template></span
          >
          <span class="caps tag">{{
            stop.root ? "Départ · Position" : "Position"
          }}</span>
        </span>
        <button
          v-else
          type="button"
          class="node"
          @click="emit('navigate', stop.path)"
        >
          <StatusIcon class="marker" name="circle" />
          <span class="label"
            >{{ stop.label }}<template v-if="stop.root"> /</template></span
          >
          <span v-if="stop.root" class="caps tag">Départ</span>
        </button>
      </li>
    </ol>
  </nav>
</template>

<style scoped>
.itinerary {
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.stops {
  display: flex;
  align-items: center;
  margin: 0;
  padding: var(--sp-2) var(--sp-4);
  overflow-x: auto;
  list-style: none;
  font-family: var(--font-mono);
  font-size: var(--fs-data);
}

.stop {
  display: flex;
  flex-shrink: 0;
  align-items: center;
}

/* Trajet entre deux étapes : tracé pointillé. */
.leg {
  width: var(--sp-5);
  margin: 0 var(--sp-2);
  border-top: var(--border-width) dashed var(--color-border-strong);
}

.node {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  padding: var(--sp-1) var(--sp-2);
  background: none;
  border: var(--border-width) solid transparent;
  border-radius: var(--r-1);
  color: var(--color-text);
  font: inherit;
  white-space: nowrap;
}

button.node {
  cursor: pointer;
  transition: background-color var(--dur-fast) var(--ease-out);
}

button.node:hover {
  background: var(--color-surface-hover);
}

.marker {
  font-size: var(--fs-xs);
  color: var(--color-text-muted);
}

.current {
  border-color: var(--color-action);
  font-weight: 600;
}

.current .marker {
  color: var(--color-action);
}

.tag {
  color: var(--color-text-muted);
}

.current .tag {
  color: var(--color-action);
}
</style>
