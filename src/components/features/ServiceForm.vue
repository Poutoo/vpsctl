<script setup lang="ts">
/**
 * Formulaire de création d'un service systemd utilisateur : identité et
 * exécution, politique de redémarrage (trois cartes), variables
 * d'environnement. Validation instantanée avec les règles du backend
 * (`serviceRules.ts`) ; le texte du fichier n'est jamais produit ici : il
 * vient du backend (aperçu à côté du formulaire).
 */
import { computed, reactive, ref, useId, watch } from "vue";
import type { RestartPolicy } from "../../bindings/RestartPolicy";
import type { ServiceDef } from "../../bindings/ServiceDef";
import { useConnectionStore } from "../../stores/connection";
import {
  descriptionProblem,
  envProblems,
  execStartProblem,
  isBlankRow,
  nameProblem,
  workingDirProblem,
  type EnvRow,
} from "../../serviceRules";
import AppButton from "../ui/AppButton.vue";
import IconButton from "../ui/IconButton.vue";
import PanelCard from "../ui/PanelCard.vue";
import StatusIcon from "../ui/StatusIcon.vue";

const props = defineProps<{
  /** Unités déjà présentes dans `~/.config/systemd/user/`. */
  taken: string[];
  /** Début de la création en cours (`performance.now()`), sinon `null`. */
  busySince: number | null;
  /** Refus du backend, affiché au-dessus des boutons. */
  error: string | null;
}>();

const connection = useConnectionStore();

const emit = defineEmits<{
  /** À chaque saisie : définition courante et premier problème éventuel. */
  change: [def: ServiceDef, problem: string | null];
  submit: [def: ServiceDef];
  cancel: [];
}>();

const POLICIES: { value: RestartPolicy; text: string }[] = [
  { value: "no", text: "Jamais relancé après arrêt ou échec." },
  { value: "on-failure", text: "Relancé uniquement si le processus échoue." },
  { value: "always", text: "Toujours relancé, même après un arrêt propre." },
];

const draft = reactive({
  name: "",
  description: "",
  execStart: "",
  workingDir: "~",
  restart: "on-failure" as RestartPolicy,
  env: [] as EnvRow[],
});

type Field = "name" | "description" | "execStart" | "workingDir";

/** Champs modifiés ou quittés : leurs problèmes s'affichent aussitôt. */
const touched = reactive<Record<Field, boolean>>({
  name: false,
  description: false,
  execStart: false,
  workingDir: false,
});
/** Un envoi a été tenté : tous les problèmes s'affichent. */
const attempted = ref(false);

const problems = computed(() => {
  const unit = `${draft.name}.service`;
  return {
    name:
      nameProblem(draft.name) ??
      (props.taken.includes(unit)
        ? `Un service porte déjà ce nom : ${unit}.`
        : null),
    description: descriptionProblem(draft.description),
    execStart: execStartProblem(draft.execStart),
    workingDir: workingDirProblem(draft.workingDir),
  };
});

const env = computed(() => envProblems(draft.env));

function shown(field: Field): string | null {
  return touched[field] || attempted.value ? problems.value[field] : null;
}

const LABELS: Record<Field, string> = {
  name: "Nom",
  description: "Description",
  execStart: "Commande",
  workingDir: "Dossier de travail",
};

/** Premier problème du formulaire, pour l'aperçu. */
const firstProblem = computed<string | null>(() => {
  for (const field of Object.keys(LABELS) as Field[]) {
    const problem = problems.value[field];
    if (problem) return `${LABELS[field]} : ${problem}`;
  }
  if (env.value.list) return `Variables d'environnement : ${env.value.list}`;
  for (const [i, row] of env.value.rows.entries()) {
    const problem = row.key ?? row.value;
    if (problem) {
      const key = draft.env[i].key || `ligne ${i + 1}`;
      return `Variable ${key} : ${problem}`;
    }
  }
  return null;
});

const def = computed<ServiceDef>(() => ({
  name: draft.name,
  description: draft.description,
  execStart: draft.execStart,
  workingDir: draft.workingDir,
  restart: draft.restart,
  env: draft.env
    .filter((row) => !isBlankRow(row))
    .map((row) => ({ key: row.key, value: row.value })),
}));

watch(
  [def, firstProblem],
  ([current, problem]) => emit("change", current, problem),
  { immediate: true },
);

function addVariable() {
  draft.env.push({ key: "", value: "" });
}

function removeVariable(index: number) {
  draft.env.splice(index, 1);
}

function submit() {
  attempted.value = true;
  if (firstProblem.value !== null || props.busySince !== null) return;
  emit("submit", def.value);
}

const uid = useId();
const helpId = (field: string) => `${uid}-${field}-help`;
const errorId = (field: string) => `${uid}-${field}-error`;

function describedBy(field: Field): string {
  return [helpId(field), shown(field) ? errorId(field) : null]
    .filter(Boolean)
    .join(" ");
}
</script>

<template>
  <form class="service-form" novalidate @submit.prevent="submit">
    <PanelCard tag="div">
      <template #kicker>Identité et exécution</template>

      <div class="field">
        <label class="label" :for="`${uid}-name`">Nom de l'unité</label>
        <div class="with-suffix">
          <input
            :id="`${uid}-name`"
            v-model="draft.name"
            type="text"
            placeholder="mon-service"
            spellcheck="false"
            autocomplete="off"
            autocapitalize="off"
            :aria-invalid="shown('name') !== null"
            :aria-describedby="describedBy('name')"
            @input="touched.name = true"
            @blur="touched.name = true"
          />
          <span class="suffix" aria-hidden="true">.service</span>
        </div>
        <span :id="helpId('name')" class="help">
          Minuscules, chiffres et tirets.
        </span>
        <span v-if="shown('name')" :id="errorId('name')" class="error">
          <StatusIcon class="shape" name="triangle" />{{ shown("name") }}
        </span>
      </div>

      <div class="field">
        <label class="label" :for="`${uid}-description`">Description</label>
        <input
          :id="`${uid}-description`"
          v-model="draft.description"
          type="text"
          class="prose"
          placeholder="Ce que fait le service"
          :aria-invalid="shown('description') !== null"
          :aria-describedby="describedBy('description')"
          @input="touched.description = true"
        />
        <span :id="helpId('description')" class="help">
          Affichée dans la liste des services. Facultative.
        </span>
        <span
          v-if="shown('description')"
          :id="errorId('description')"
          class="error"
        >
          <StatusIcon class="shape" name="triangle" />{{ shown("description") }}
        </span>
      </div>

      <div class="field">
        <label class="label" :for="`${uid}-exec`">Commande</label>
        <input
          :id="`${uid}-exec`"
          v-model="draft.execStart"
          type="text"
          placeholder="/usr/bin/node dist/server.js"
          spellcheck="false"
          autocomplete="off"
          autocapitalize="off"
          :aria-invalid="shown('execStart') !== null"
          :aria-describedby="describedBy('execStart')"
          @input="touched.execStart = true"
          @blur="touched.execStart = true"
        />
        <span :id="helpId('execStart')" class="help">
          <span class="verbatim">ExecStart</span> · chemin absolu de
          l'exécutable, puis ses arguments.
          <span class="subtle">
            Syntaxe de systemd, pas un shell : guillemets pour grouper, pas de
            <span class="verbatim">|</span>,
            <span class="verbatim">&amp;&amp;</span> ni
            <span class="verbatim">&gt;</span>. Pour un shell :
            <span class="verbatim">/bin/sh -c '…'</span> (les
            <span class="verbatim">$VAR</span> y sont développés par le shell).
          </span>
        </span>
        <span
          v-if="shown('execStart')"
          :id="errorId('execStart')"
          class="error"
        >
          <StatusIcon class="shape" name="triangle" />{{ shown("execStart") }}
        </span>
      </div>

      <div class="field">
        <label class="label" :for="`${uid}-dir`">Dossier de travail</label>
        <input
          :id="`${uid}-dir`"
          v-model="draft.workingDir"
          type="text"
          placeholder="/srv/mon-service"
          spellcheck="false"
          autocomplete="off"
          autocapitalize="off"
          :aria-invalid="shown('workingDir') !== null"
          :aria-describedby="describedBy('workingDir')"
          @input="touched.workingDir = true"
          @blur="touched.workingDir = true"
        />
        <span :id="helpId('workingDir')" class="help">
          <span class="verbatim">WorkingDirectory</span> · chemin absolu, ou
          commençant par <span class="verbatim">~/</span>.
          <span class="verbatim">~</span> seul : dossier personnel.
        </span>
        <span
          v-if="shown('workingDir')"
          :id="errorId('workingDir')"
          class="error"
        >
          <StatusIcon class="shape" name="triangle" />{{ shown("workingDir") }}
        </span>
      </div>
    </PanelCard>

    <PanelCard tag="div">
      <template #kicker>Politique de redémarrage</template>
      <template #aside>RestartSec=5</template>
      <fieldset class="policies">
        <legend class="visually-hidden">Politique de redémarrage</legend>
        <label
          v-for="policy in POLICIES"
          :key="policy.value"
          class="policy"
          :class="{ on: draft.restart === policy.value }"
        >
          <input
            v-model="draft.restart"
            type="radio"
            name="restart"
            :value="policy.value"
          />
          <span class="policy-text">
            <span class="policy-name">{{ policy.value }}</span>
            <span class="policy-help">{{ policy.text }}</span>
          </span>
        </label>
      </fieldset>
    </PanelCard>

    <PanelCard tag="div">
      <template #kicker>Variables d'environnement</template>
      <template #aside>
        <AppButton size="compact" @click="addVariable">Ajouter</AppButton>
      </template>
      <p v-if="draft.env.length === 0" class="help">
        Aucune variable. Elles sont transmises au processus, une ligne
        <span class="verbatim">Environment=</span> chacune.
      </p>
      <ul v-else class="variables">
        <li v-for="(row, i) in draft.env" :key="i" class="variable">
          <div class="pair">
            <input
              v-model="row.key"
              type="text"
              placeholder="CLÉ"
              spellcheck="false"
              autocomplete="off"
              autocapitalize="off"
              :aria-label="`Clé de la variable ${i + 1}`"
              :aria-invalid="env.rows[i]?.key !== null"
            />
            <span class="equals" aria-hidden="true">=</span>
            <input
              v-model="row.value"
              type="text"
              placeholder="valeur"
              spellcheck="false"
              autocomplete="off"
              autocapitalize="off"
              :aria-label="`Valeur de la variable ${row.key || i + 1}`"
              :aria-invalid="env.rows[i]?.value !== null"
            />
            <IconButton
              icon="close"
              label="Retirer la variable"
              @click="removeVariable(i)"
            />
          </div>
          <span
            v-if="env.rows[i]?.key ?? env.rows[i]?.value"
            class="error"
            role="status"
          >
            <StatusIcon class="shape" name="triangle" />{{
              env.rows[i]?.key ?? env.rows[i]?.value
            }}
          </span>
        </li>
      </ul>
      <p v-if="env.list" class="error" role="status">
        <StatusIcon class="shape" name="triangle" />{{ env.list }}
      </p>
    </PanelCard>

    <p v-if="error" class="error submit-error" role="alert">
      <StatusIcon class="shape" name="triangle" />{{ error }}
    </p>

    <div class="buttons">
      <AppButton :disabled="busySince !== null" @click="emit('cancel')">
        Annuler
      </AppButton>
      <AppButton
        variant="primary"
        type="submit"
        :disabled="connection.lost"
        :busy-since="busySince"
      >
        {{ busySince !== null ? "Création" : "Créer le service" }}
      </AppButton>
    </div>
  </form>
</template>

<style scoped>
.service-form {
  display: flex;
  flex-direction: column;
  gap: var(--sp-5);
  min-width: 0;
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

/* Description : texte courant, pas une donnée technique. */
.prose {
  font-family: var(--font-ui);
}

/* Nom : suffixe .service accolé au champ. */
.with-suffix {
  display: flex;
  min-width: 0;
}

.with-suffix input {
  border-top-right-radius: 0;
  border-bottom-right-radius: 0;
}

.suffix {
  display: inline-flex;
  flex-shrink: 0;
  align-items: center;
  padding: 0 var(--sp-3);
  background: var(--color-surface-raised);
  border: var(--border-width) solid var(--color-border-strong);
  border-left: none;
  border-radius: 0 var(--r-1) var(--r-1) 0;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-md);
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

.verbatim {
  font-family: var(--font-mono);
}

.error {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  margin: 0;
  color: var(--color-danger-text);
  font-size: var(--fs-sm);
}

.shape {
  flex-shrink: 0;
  color: var(--color-danger);
}

/* Politique de redémarrage : trois cartes, la choisie bordée d'ambre. */
.policies {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--sp-3);
  margin: 0;
  padding: 0;
  border: none;
}

.policy {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-2);
  padding: var(--sp-3);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border-strong);
  cursor: pointer;
  transition:
    background-color var(--dur-fast) var(--ease-out),
    border-color var(--dur-fast) var(--ease-out);
}

.policy:hover {
  border-color: var(--color-text-subtle);
}

.policy.on {
  background: var(--color-selection);
  border-color: var(--color-action);
}

.policy input {
  margin-top: var(--sp-1);
}

.policy-text {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.policy-name {
  font-family: var(--font-mono);
  font-size: var(--fs-md);
  font-weight: 600;
}

.policy-help {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

/* Variables : CLÉ = valeur, et le bouton pour retirer la ligne. */
.variables {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.variable {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
}

.pair {
  display: grid;
  grid-template-columns: minmax(0, 2fr) auto minmax(0, 3fr) auto;
  align-items: center;
  gap: var(--sp-3);
}

.equals {
  color: var(--color-text-muted);
  font-family: var(--font-mono);
}

.submit-error {
  padding: var(--sp-3) var(--sp-4);
  background: var(--color-danger-tint);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-danger);
}

.buttons {
  display: flex;
  justify-content: flex-end;
  gap: var(--sp-3);
}
</style>
