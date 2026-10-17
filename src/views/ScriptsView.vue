<script setup lang="ts">
/**
 * Scripts : liste des procédures enregistrées localement, exécution (GO /
 * NO GO si le script l'exige, sortie dans un onglet de l'écran
 * Télémétrie), création, modification et suppression.
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { errorMessage, toAppError } from "../api/errors";
import type { Script } from "../bindings/Script";
import ScriptForm from "../components/features/ScriptForm.vue";
import ScriptTable from "../components/features/ScriptTable.vue";
import PageHeader from "../components/layout/PageHeader.vue";
import AppButton from "../components/ui/AppButton.vue";
import StateMessage from "../components/ui/StateMessage.vue";
import { useConfirm } from "../composables/useConfirm";
import { useScriptActions } from "../composables/useScriptActions";
import { useScriptsStore } from "../stores/scripts";
import { useTelemetryStore } from "../stores/telemetry";

/** Rafraîchissement de l'ancienneté affichée (« il y a 5 min »). */
const CLOCK_MS = 30_000;

const store = useScriptsStore();
const telemetry = useTelemetryStore();
const { run } = useScriptActions();
const { confirm } = useConfirm();

/** Script édité : identifiant, chaîne vide pour un nouveau, `null` : la liste. */
const editedId = ref<string | null>(null);
const busy = ref(false);
const formError = ref<string | null>(null);
const loadError = ref<string | null>(null);

const now = ref(Date.now());
let clock: ReturnType<typeof setInterval> | null = null;

onMounted(async () => {
  clock = setInterval(() => (now.value = Date.now()), CLOCK_MS);
  try {
    await store.load();
  } catch (e) {
    loadError.value = errorMessage(toAppError(e));
  }
});

onBeforeUnmount(() => {
  if (clock) clearInterval(clock);
});

function blankScript(): Script {
  return {
    id: "",
    name: "",
    description: "",
    command: "",
    requireConfirmation: true,
    lastRun: null,
  };
}

const edited = computed<Script | null>(() => {
  if (editedId.value === "") return blankScript();
  return store.byId(editedId.value);
});

const countLabel = computed(() => {
  const count = store.scripts.length;
  return `${count} ${count > 1 ? "scripts" : "script"}`;
});

function edit(id: string) {
  formError.value = null;
  editedId.value = id;
}

async function save(script: Script) {
  busy.value = true;
  formError.value = null;
  try {
    await store.save(script);
    editedId.value = null;
  } catch (e) {
    formError.value = errorMessage(toAppError(e));
  } finally {
    busy.value = false;
  }
}

async function remove(script: Script) {
  const running = telemetry.scriptTab(script.id)?.session.active ?? false;
  const confirmed = await confirm({
    title: "Supprimer le script ?",
    subject: script.name,
    consequences: [
      "Le script est retiré de l'application, avec sa dernière exécution.",
      "Rien n'est modifié sur le serveur : un fichier appelé par la commande reste en place.",
      ...(running
        ? [
            "Son exécution en cours continue jusqu'à sa fin ; elle ne sera pas enregistrée.",
          ]
        : []),
    ],
    confirmLabel: "Supprimer",
    danger: true,
  });
  if (!confirmed) return;
  try {
    await store.remove(script.id);
    loadError.value = null;
  } catch (e) {
    loadError.value = errorMessage(toAppError(e));
  }
}
</script>

<template>
  <div class="scripts">
    <PageHeader section="scripts" title="Scripts">
      <template v-if="edited === null">
        <span class="count">{{ countLabel }}</span>
        <AppButton variant="primary" @click="edit('')"
          >Nouveau script</AppButton
        >
      </template>
    </PageHeader>

    <ScriptForm
      v-if="edited"
      :key="edited.id || 'nouveau'"
      class="form"
      :script="edited"
      :busy="busy"
      :error="formError"
      @save="save"
      @cancel="editedId = null"
    />

    <template v-else>
      <StateMessage
        v-if="loadError"
        tone="error"
        kicker="scripts.json"
        title="Scripts illisibles"
      >
        {{ loadError }}
      </StateMessage>
      <StateMessage
        v-else-if="store.loaded && store.scripts.length === 0"
        kicker="0 procédure"
        title="Aucun script"
      >
        Enregistrez une commande à exécuter sur le serveur : déploiement,
        sauvegarde, nettoyage…
        <template #actions>
          <AppButton variant="primary" @click="edit('')">
            Nouveau script
          </AppButton>
        </template>
      </StateMessage>
      <ScriptTable
        v-else-if="store.scripts.length > 0"
        :scripts="store.scripts"
        :now="now"
        @run="run"
        @edit="(script) => edit(script.id)"
        @remove="remove"
      />

      <p class="note">
        Les scripts sont enregistrés dans l'application. Une procédure marquée
        GO / NO GO demande une confirmation avant exécution ; sa sortie
        s'affiche dans la console de télémétrie.
      </p>
    </template>
  </div>
</template>

<style scoped>
.scripts {
  display: flex;
  flex-direction: column;
  gap: var(--sp-5);
}

.count {
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

.form {
  max-width: calc(11 * var(--sp-8));
}

.note {
  max-width: 80ch;
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}
</style>
