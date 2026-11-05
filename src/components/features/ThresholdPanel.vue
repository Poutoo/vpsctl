<script setup lang="ts">
/**
 * Seuils d'alerte du profil affiché (planche VII), par ordre de priorité :
 * disque, mémoire, charge. Chaque saisie est contrôlée aussitôt
 * (`resourceRules`, mêmes règles que le backend) et enregistrée 300 ms après
 * la dernière frappe ; une saisie invalide n'est pas enregistrée et les
 * alertes gardent les seuils précédents. « Valeurs par défaut » ne touche que
 * ce profil.
 */
import { computed, onBeforeUnmount, reactive, watch } from "vue";
import { errorMessage, toAppError } from "../../api/errors";
import type { Thresholds } from "../../bindings/Thresholds";
import {
  parseThreshold,
  sameThresholds,
  thresholdProblems,
  type Resource,
  type ThresholdField,
} from "../../resourceRules";
import { formatLoadThreshold } from "../../resourceText";
import { useConnectionStore } from "../../stores/connection";
import { useProfilesStore } from "../../stores/profiles";
import { useResourcesStore } from "../../stores/resources";
import AppButton from "../ui/AppButton.vue";
import StatusIcon from "../ui/StatusIcon.vue";

const SAVE_DELAY_MS = 300;

const store = useResourcesStore();
const connection = useConnectionStore();
const profiles = useProfilesStore();

const profileName = computed(() => profiles.byId(store.profileId)?.name ?? "");

const ROWS: {
  resource: Resource;
  label: string;
  hint: string;
  unit: string;
}[] = [
  {
    resource: "disk",
    label: "Disque",
    hint: "Occupation d'un point de montage",
    unit: "%",
  },
  {
    resource: "memory",
    label: "Mémoire",
    hint: "Mémoire vive utilisée",
    unit: "%",
  },
  {
    resource: "load",
    label: "Charge",
    hint: "Moyenne sur 5 min, par cœur",
    unit: "× cœurs",
  },
];

/** Texte saisi de chaque champ. */
const draft = reactive<Record<ThresholdField, string>>({
  "disk.attention": "",
  "disk.critical": "",
  "memory.attention": "",
  "memory.critical": "",
  "load.attention": "",
  "load.critical": "",
});
const state = reactive({ error: null as string | null });
/** Saisie différente des seuils enregistrés (en cours, ou invalide). */
let dirty = false;
let timer: ReturnType<typeof setTimeout> | null = null;

function display(resource: Resource, value: number): string {
  return resource === "load" ? formatLoadThreshold(value) : String(value);
}

function fill(thresholds: Thresholds) {
  for (const resource of ["disk", "memory", "load"] as const) {
    draft[`${resource}.attention`] = display(
      resource,
      thresholds[resource].attention,
    );
    draft[`${resource}.critical`] = display(
      resource,
      thresholds[resource].critical,
    );
  }
}

const parsed = computed<Thresholds>(() => {
  const value = (field: ThresholdField) => parseThreshold(draft[field]);
  return {
    disk: {
      attention: value("disk.attention"),
      critical: value("disk.critical"),
    },
    memory: {
      attention: value("memory.attention"),
      critical: value("memory.critical"),
    },
    load: {
      attention: value("load.attention"),
      critical: value("load.critical"),
    },
  };
});
const problems = computed(() => thresholdProblems(parsed.value));
const valid = computed(() =>
  Object.values(problems.value).every((p) => p === null),
);

// Seuils reçus (chargement, valeurs par défaut) : la saisie suit, sauf si
// elle diffère encore des seuils enregistrés (frappe en cours, valeur
// invalide gardée sous les yeux de l'utilisateur).
watch(
  () => store.thresholds,
  (thresholds) => {
    if (thresholds && !dirty) fill(thresholds);
  },
  { immediate: true },
);
watch(
  () => store.profileId,
  () => {
    dirty = false;
    if (store.thresholds) fill(store.thresholds);
  },
);

async function save() {
  timer = null;
  if (!valid.value || !store.thresholds) return;
  if (sameThresholds(store.thresholds, parsed.value)) {
    dirty = false;
    return;
  }
  try {
    await store.saveThresholds(parsed.value);
    dirty = timer !== null;
    state.error = null;
  } catch (e) {
    state.error = errorMessage(toAppError(e));
  }
}

function onInput() {
  dirty = true;
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => void save(), SAVE_DELAY_MS);
}

async function reset() {
  if (timer) clearTimeout(timer);
  timer = null;
  dirty = false;
  try {
    await store.resetThresholds();
    if (store.thresholds) fill(store.thresholds);
    state.error = null;
  } catch (e) {
    state.error = errorMessage(toAppError(e));
  }
}

onBeforeUnmount(() => {
  if (timer) {
    clearTimeout(timer);
    void save();
  }
});

const fieldId = (field: ThresholdField) =>
  `threshold-${field.replace(".", "-")}`;
</script>

<template>
  <section class="panel" data-testid="threshold-panel">
    <header class="head">
      <h2 class="caps">Seuils d'alerte · par ordre de priorité</h2>
      <AppButton
        size="compact"
        :disabled="!store.thresholds"
        data-testid="thresholds-reset"
        @click="reset"
        >Valeurs par défaut</AppButton
      >
    </header>
    <p class="profile">
      <span class="caps">Profil</span>
      <span class="mono name" data-testid="thresholds-profile">{{
        profileName
      }}</span>
      <span class="hint"
        >Ces seuils ne s'appliquent qu'à ce serveur. Chaque profil a les siens,
        et « Valeurs par défaut » ne rétablit que ceux-ci.</span
      >
    </p>

    <div v-for="(row, index) in ROWS" :key="row.resource" class="row">
      <div class="label">
        <p>
          <span class="rank mono">{{ index + 1 }}</span>
          <strong>{{ row.label }}</strong>
        </p>
        <p class="hint">{{ row.hint }}</p>
      </div>
      <div
        v-for="level in ['attention', 'critical'] as const"
        :key="level"
        class="field"
      >
        <label
          class="caps level"
          :class="level"
          :for="fieldId(`${row.resource}.${level}`)"
        >
          <StatusIcon :name="level === 'attention' ? 'triangle' : 'circle'" />
          {{ level === "attention" ? "Attention" : "Critique" }}
        </label>
        <input
          :id="fieldId(`${row.resource}.${level}`)"
          v-model="draft[`${row.resource}.${level}`]"
          type="text"
          inputmode="decimal"
          class="mono"
          :data-testid="`threshold-${row.resource}-${level}`"
          :aria-invalid="problems[`${row.resource}.${level}`] !== null"
          @input="onInput"
        />
        <span class="unit mono">{{ row.unit }}</span>
        <span
          v-if="problems[`${row.resource}.${level}`]"
          class="error"
          :data-testid="`threshold-error-${row.resource}-${level}`"
        >
          {{ problems[`${row.resource}.${level}`] }}
        </span>
      </div>
    </div>
    <p v-if="state.error" class="save-error">{{ state.error }}</p>

    <div class="explain">
      <svg
        class="figure"
        viewBox="0 0 180 60"
        aria-hidden="true"
        focusable="false"
      >
        <line class="t-raise" x1="0" y1="14" x2="180" y2="14" />
        <line class="t-fall" x1="0" y1="24" x2="180" y2="24" />
        <polyline
          class="curve"
          points="0,48 30,34 50,22 60,10 75,20 90,12 110,22 125,30 150,40 180,44"
        />
        <line class="active" x1="50" y1="56" x2="125" y2="56" />
      </svg>
      <ul class="figure-keys mono">
        <li><span class="k raise" />se lève au seuil</li>
        <li><span class="k fall" />retombe sous la marge</li>
        <li><span class="k on" />alerte active</li>
      </ul>
      <div class="text">
        <p>
          Une alerte se lève dès que le seuil est atteint et ne retombe qu'après
          3 points en dessous (0,1 par cœur pour la charge) : pas de
          clignotement autour du seuil.
        </p>
        <p>
          Les relevés se font toutes les 5 s, écran ouvert. Disque et mémoire
          sont aussi relevés chaque minute hors de cet écran, pour alimenter le
          repère de navigation.
        </p>
        <p>
          Les courbes couvrent la session et disparaissent à la fermeture
          d'Atlas. Rien n'est installé sur le serveur.
        </p>
        <p v-if="connection.lost">
          Liaison perdue : les seuils restent modifiables.
        </p>
      </div>
    </div>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  min-height: var(--row-height);
  padding: var(--sp-2) var(--sp-4);
  background: var(--color-surface-raised);
  border-bottom: var(--border-width) solid var(--color-border);
}

.head h2 {
  color: var(--color-text-muted);
}

.profile {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--sp-2) var(--sp-4);
  margin: 0 var(--sp-4);
  padding: var(--sp-3) 0;
  border-bottom: var(--border-width) solid var(--color-border);
}

.profile .caps {
  color: var(--color-text-muted);
}

.name {
  font-size: var(--fs-data);
}

.hint {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.row {
  display: grid;
  grid-template-columns: minmax(0, 4fr) minmax(0, 3fr) minmax(0, 3fr);
  align-items: start;
  gap: var(--sp-4);
  margin: 0 var(--sp-4);
  padding: var(--sp-3) 0;
  border-bottom: var(--border-width) solid var(--color-border);
}

.rank {
  margin-right: var(--sp-2);
  color: var(--color-text-muted);
}

.field {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--sp-1) var(--sp-2);
}

.field input {
  width: 100%;
}

.level {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-1);
  text-transform: none;
}

.level.attention {
  color: var(--color-warning);
}

.level.critical {
  color: var(--color-danger-text);
}

.unit {
  color: var(--color-text-muted);
  font-size: var(--fs-xs);
}

.error,
.save-error {
  grid-column: 2 / -1;
  color: var(--color-danger-text);
  font-size: var(--fs-sm);
}

.save-error {
  margin: var(--sp-2) var(--sp-4) 0;
}

.explain {
  display: grid;
  grid-template-columns: minmax(0, 2fr) minmax(0, 3fr);
  gap: var(--sp-3) var(--sp-5);
  padding: var(--sp-4);
}

/* Figure de l'hystérésis : taille de la maquette, sans grandir avec la colonne. */
.figure {
  width: 100%;
  max-width: calc(3 * var(--sp-8));
}

.figure-keys {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-1) var(--sp-3);
  margin: 0;
  padding: 0;
  list-style: none;
  color: var(--color-text-muted);
  font-size: var(--fs-xs);
}

.figure-keys li {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-1);
}

.k {
  display: inline-block;
  width: var(--sp-4);
  border-top: var(--border-width) dashed var(--color-warning);
}

.k.fall {
  border-top-style: dotted;
  border-top-color: var(--color-text-muted);
}

.k.on {
  border-top: var(--border-width-accent) solid var(--color-warning);
}

.t-raise {
  stroke: var(--color-warning);
  stroke-dasharray: 6 4;
  stroke-width: var(--border-width);
}

.t-fall {
  stroke: var(--color-text-muted);
  stroke-dasharray: 1 3;
  stroke-width: var(--border-width);
}

.curve {
  fill: none;
  stroke: var(--color-text);
  stroke-width: var(--border-width-accent);
}

.active {
  stroke: var(--color-warning);
  stroke-width: var(--border-width-accent);
}

.text {
  grid-column: 2;
  grid-row: 1 / span 2;
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}
</style>
