<script setup lang="ts">
/**
 * Création d'un service systemd utilisateur, ouverte depuis le tableau de
 * bord (la navigation surligne Tableau de bord). Formulaire à gauche, aperçu
 * du fichier à droite ; avertissement si le lingering n'est pas actif.
 *
 * Après la création, retour au tableau de bord : le service y est « en
 * transition », puis son état est vérifié par le store `services`.
 */
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { errorMessage, toAppError } from "../api/errors";
import { userLinger } from "../api/systemd";
import type { ServiceDef } from "../bindings/ServiceDef";
import ServiceForm from "../components/features/ServiceForm.vue";
import UnitPreview from "../components/features/UnitPreview.vue";
import PageHeader from "../components/layout/PageHeader.vue";
import StatusIcon from "../components/ui/StatusIcon.vue";
import { useNotify } from "../composables/useNotify";
import { SECTIONS } from "../navigation";
import { useServicesStore } from "../stores/services";
import { serviceLogTarget, useTelemetryStore } from "../stores/telemetry";

const store = useServicesStore();
const telemetry = useTelemetryStore();
const router = useRouter();
const { notify } = useNotify();

const kicker = `${SECTIONS.find((s) => s.route === "dashboard")?.numeral} · Nouvelle unité`;

/** Lingering du compte : `null` tant qu'il est inconnu. */
const linger = ref<boolean | null>(null);
const lingerKnown = ref(false);

onMounted(async () => {
  // Liste des services : pour signaler tout de suite un nom déjà pris.
  if (!store.services.loaded) void store.refresh();
  try {
    linger.value = await userLinger();
  } catch {
    linger.value = null;
  }
  lingerKnown.value = true;
});

const taken = computed(() => store.services.items.map((s) => s.unit));

const def = ref<ServiceDef | null>(null);
const problem = ref<string | null>(null);

function onChange(current: ServiceDef, firstProblem: string | null) {
  def.value = current;
  problem.value = firstProblem;
}

const busySince = ref<number | null>(null);
const error = ref<string | null>(null);

function backToDashboard() {
  void router.push({ name: "dashboard" });
}

async function create(current: ServiceDef) {
  busySince.value = performance.now();
  error.value = null;
  try {
    await store.create(current);
    backToDashboard();
  } catch (raw) {
    busySince.value = null;
    const failure = toAppError(raw);
    if (failure.kind === "ServiceStepFailed") {
      // Le fichier est écrit : le service existe, il est affiché en échec
      // ou arrêté sur le tableau de bord.
      notify({
        tone: "error",
        title: "Service créé, mais non démarré",
        detail: failure.unit,
        message: errorMessage(failure),
        action: {
          label: "Voir la télémétrie",
          run: () => telemetry.showLogs(serviceLogTarget(failure.unit)),
        },
      });
      void store.refresh();
      backToDashboard();
      return;
    }
    error.value = errorMessage(failure);
  }
}
</script>

<template>
  <div class="create">
    <PageHeader :kicker="kicker" title="Créer un service systemd">
      <span class="target">~/.config/systemd/user/</span>
    </PageHeader>

    <p v-if="lingerKnown && linger === false" class="warning" role="status">
      <StatusIcon class="warning-shape" name="triangle" />
      <span>
        <strong>Lingering inactif pour ce compte.</strong>
        Sans lui, le service s'arrêtera à la fin de la dernière session SSH du
        compte. À activer sur le VPS avec
        <span class="verbatim">loginctl enable-linger</span>.
      </span>
    </p>
    <p v-else-if="lingerKnown && linger === null" class="note" role="status">
      État du lingering inconnu : sans lui, le service s'arrêtera à la fin de la
      dernière session du compte.
    </p>

    <div class="layout">
      <ServiceForm
        :taken="taken"
        :busy-since="busySince"
        :error="error"
        @change="onChange"
        @submit="create"
        @cancel="backToDashboard"
      />
      <UnitPreview v-if="def" class="aside" :def="def" :problem="problem" />
    </div>
  </div>
</template>

<style scoped>
.create {
  display: flex;
  flex-direction: column;
  gap: var(--sp-5);
}

.target {
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

/* Formulaire à gauche, aperçu à droite (il reste visible au défilement). */
.layout {
  display: grid;
  grid-template-columns: minmax(0, 7fr) minmax(0, 4fr);
  align-items: start;
  gap: var(--sp-5);
}

.aside {
  position: sticky;
  top: 0;
}

.warning {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-2);
  margin: 0;
  padding: var(--sp-3) var(--sp-4);
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-warning);
  font-size: var(--fs-sm);
}

.warning-shape {
  flex-shrink: 0;
  margin-top: var(--sp-1);
  color: var(--color-warning);
}

.verbatim {
  font-family: var(--font-mono);
}

.note {
  margin: 0;
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}
</style>
