<script setup lang="ts">
/**
 * Itinéraire : fil d'Ariane du dossier affiché, de la racine (DÉPART, au nom
 * du serveur) jusqu'à la POSITION. Chaque étape parcourue est un bouton qui
 * y revient.
 *
 * Sans barre de défilement : quand le chemin est trop long, la racine et la
 * position restent affichées, avec autant d'étapes qui précèdent la position
 * que la place le permet ; les autres sont regroupées dans « [ … ] », qui
 * ouvre la liste des dossiers masqués. Les largeurs sont mesurées sur une
 * rangée invisible (polices et tailles réelles) et la place disponible suivie
 * par un ResizeObserver.
 */
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuTrigger,
} from "reka-ui";
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { timeLabel } from "../../linkText";
import { pathFromSegments, pathSegments } from "../../remoteFiles";
import AppIcon from "../ui/AppIcon.vue";
import StatusIcon from "../ui/StatusIcon.vue";

const props = defineProps<{
  path: string;
  /** Nom du serveur, affiché à la racine. */
  server: string;
  /** Lecture en cours ou LOS : les étapes ne sont plus cliquables. */
  busy?: boolean;
  /** LOS : contenu figé depuis cette heure. */
  frozenAt?: number | null;
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

// --- Repli des étapes en trop --------------------------------------------------

/** Rangée affichée : sa largeur utile est la place disponible. */
const track = ref<HTMLElement | null>(null);
/** Rangée invisible où chaque étape est mesurée. */
const ruler = ref<HTMLElement | null>(null);
const available = ref(Number.POSITIVE_INFINITY);
/** Largeur de chaque étape, tracé qui la précède compris. */
const widths = ref<number[]>([]);
/** Largeur de « [ … ] », tracé compris. */
const moreWidth = ref(0);

function measure() {
  const element = ruler.value;
  if (!element) return;
  widths.value = Array.from(
    element.querySelectorAll<HTMLElement>("[data-stop]"),
    (item) => item.getBoundingClientRect().width,
  );
  moreWidth.value =
    element.querySelector<HTMLElement>("[data-more]")?.getBoundingClientRect()
      .width ?? 0;
}

/**
 * Index de la première étape affichée après la racine : les étapes 1 à
 * `firstShown - 1` sont repliées dans « [ … ] ». La position (dernière
 * étape) est toujours affichée ; trop longue, son nom est tronqué.
 */
const firstShown = computed(() => {
  const all = widths.value;
  const last = stops.value.length - 1;
  if (all.length !== stops.value.length || last < 2) return 1;
  const room = available.value;
  if (all.reduce((sum, width) => sum + width, 0) <= room) return 1;
  let used = all[0] + moreWidth.value + all[last];
  let first = last;
  while (first - 1 >= 2 && used + all[first - 1] <= room) {
    first -= 1;
    used += all[first];
  }
  return first;
});

const hidden = computed(() => stops.value.slice(1, firstShown.value));
const shown = computed(() => stops.value.slice(firstShown.value));

let observer: ResizeObserver | null = null;

onMounted(() => {
  observer = new ResizeObserver(([entry]) => {
    available.value = entry.contentRect.width;
  });
  if (track.value) observer.observe(track.value);
  measure();
  // Polices embarquées chargées après le premier rendu : nouvelles largeurs.
  void document.fonts.ready.then(measure);
});

onBeforeUnmount(() => observer?.disconnect());

watch(stops, () => void nextTick(measure), { flush: "post" });
</script>

<template>
  <nav class="itinerary" aria-label="Itinéraire">
    <ol ref="track" class="stops">
      <li class="stop root">
        <span
          v-if="stops.length === 1"
          class="node current"
          aria-current="location"
        >
          <StatusIcon class="marker" name="disc" />
          <span class="label">{{ stops[0].label }} /</span>
          <span class="caps tag">Départ · Position</span>
        </span>
        <button
          v-else
          type="button"
          class="node"
          :disabled="busy"
          @click="emit('navigate', stops[0].path)"
        >
          <StatusIcon class="marker" name="circle" />
          <span class="label">{{ stops[0].label }} /</span>
          <span class="caps tag">Départ</span>
        </button>
      </li>

      <li v-if="hidden.length > 0" class="stop">
        <span class="leg" aria-hidden="true" />
        <DropdownMenuRoot>
          <DropdownMenuTrigger as-child>
            <button
              type="button"
              class="node more"
              :disabled="busy"
              :aria-label="
                hidden.length === 1
                  ? '1 dossier masqué'
                  : `${hidden.length} dossiers masqués`
              "
            >
              [ … ]
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuPortal>
            <DropdownMenuContent
              class="app-menu"
              side="bottom"
              align="start"
              :side-offset="4"
            >
              <DropdownMenuLabel class="app-menu-label"
                >Dossiers masqués</DropdownMenuLabel
              >
              <DropdownMenuItem
                v-for="stop in hidden"
                :key="stop.path"
                class="app-menu-item"
                :title="stop.path"
                @select="emit('navigate', stop.path)"
              >
                <span class="hidden-stop">
                  <StatusIcon class="marker" name="circle" />
                  {{ stop.label }}
                </span>
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenuPortal>
        </DropdownMenuRoot>
      </li>

      <li
        v-for="(stop, index) in shown"
        :key="stop.path"
        class="stop"
        :class="{ last: index === shown.length - 1 }"
      >
        <span class="leg" aria-hidden="true" />
        <span
          v-if="index === shown.length - 1"
          class="node current"
          aria-current="location"
          :title="stop.path"
        >
          <StatusIcon class="marker" name="disc" />
          <span class="label">{{ stop.label }}</span>
          <span class="caps tag">Position</span>
        </span>
        <button
          v-else
          type="button"
          class="node"
          :disabled="busy"
          @click="emit('navigate', stop.path)"
        >
          <StatusIcon class="marker" name="circle" />
          <span class="label">{{ stop.label }}</span>
        </button>
      </li>
    </ol>

    <!-- Mesure : toutes les étapes, telles qu'affichées, hors de la vue. -->
    <ol ref="ruler" class="stops ruler" aria-hidden="true">
      <li
        v-for="(stop, index) in stops"
        :key="stop.path"
        class="stop"
        data-stop
      >
        <span v-if="index > 0" class="leg" />
        <span class="node" :class="{ current: index === stops.length - 1 }">
          <StatusIcon class="marker" name="circle" />
          <span class="label"
            >{{ stop.label }}<template v-if="stop.root"> /</template></span
          >
          <span v-if="stop.root || index === stops.length - 1" class="caps tag">
            {{
              stop.root && index === stops.length - 1
                ? "Départ · Position"
                : stop.root
                  ? "Départ"
                  : "Position"
            }}
          </span>
        </span>
      </li>
      <li class="stop" data-more>
        <span class="leg" />
        <span class="node more">[ … ]</span>
      </li>
    </ol>
    <span v-if="frozenAt" class="caps frozen">
      <AppIcon name="pause" />
      Figé · {{ timeLabel(frozenAt) }}
    </span>
  </nav>
</template>

<style scoped>
/* LOS : pastille à droite de l'itinéraire, sur la surface de la barre. */
.frozen {
  position: absolute;
  top: 50%;
  right: var(--sp-4);
  display: inline-flex;
  align-items: center;
  gap: var(--sp-1);
  padding-left: var(--sp-3);
  background: var(--color-surface);
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  transform: translateY(-50%);
}

.itinerary {
  position: relative;
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.stops {
  display: flex;
  align-items: center;
  margin: 0;
  padding: var(--sp-2) var(--sp-4);
  overflow: hidden;
  list-style: none;
  font-family: var(--font-mono);
  font-size: var(--fs-data);
}

/* Rangée de mesure : mêmes styles, invisible, sans effet sur la mise en page. */
.ruler {
  position: absolute;
  top: 0;
  left: 0;
  height: 0;
  padding: 0;
  overflow: hidden;
  visibility: hidden;
  pointer-events: none;
}

.stop {
  display: flex;
  flex-shrink: 0;
  align-items: center;
}

/* La position peut se tronquer si même seule elle manque de place. */
.stop.last {
  flex-shrink: 1;
  min-width: 0;
}

/* Trajet entre deux étapes : tracé pointillé. */
.leg {
  flex-shrink: 0;
  width: var(--sp-5);
  margin: 0 var(--sp-2);
  border-top: var(--border-width) dashed var(--color-border-strong);
}

.node {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  min-width: 0;
  padding: var(--sp-1) var(--sp-2);
  background: none;
  border: var(--border-width) solid transparent;
  border-radius: var(--r-1);
  color: var(--color-text);
  font: inherit;
  white-space: nowrap;
}

.label {
  overflow: hidden;
  text-overflow: ellipsis;
}

button.node {
  cursor: pointer;
  transition: background-color var(--dur-fast) var(--ease-out);
}

button.node:hover:not(:disabled),
button.node[data-state="open"] {
  background: var(--color-surface-hover);
}

button.node:disabled {
  color: var(--color-text-subtle);
  cursor: progress;
}

/* « [ … ] » : étapes repliées, en texte discret. */
.more {
  color: var(--color-text-muted);
  letter-spacing: var(--tracking-caps);
}

.marker {
  flex-shrink: 0;
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
  flex-shrink: 0;
  color: var(--color-text-muted);
}

.current .tag {
  color: var(--color-action);
}

/* Liste des dossiers masqués (menu, rendu dans un portail). */
.hidden-stop {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  font-family: var(--font-mono);
  font-size: var(--fs-data);
  font-weight: 500;
}
</style>
