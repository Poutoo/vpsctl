<script setup lang="ts">
/**
 * Panneau « Discord Rich Presence » des préférences (planche VI) :
 * interrupteur, ce qui est partagé ou jamais partagé, état de la connexion
 * à Discord et aperçu de la carte.
 */
import { computed, onMounted, ref, watch } from "vue";
import type { LinkPhase } from "../../bindings/LinkPhase";
import { formatClock, useElapsed } from "../../composables/useElapsed";
import {
  APP_NAME,
  PHASE_LABEL,
  SOURCE_LABEL,
  presenceBadge,
  presenceNotice,
  previewKicker,
} from "../../presenceText";
import { usePresenceStore } from "../../stores/presence";
import symbol from "../../assets/logo/atlas-symbole.svg?no-inline";
import AppButton from "../ui/AppButton.vue";
import AppIcon from "../ui/AppIcon.vue";
import AppSwitch from "../ui/AppSwitch.vue";
import NoticeBox from "../ui/NoticeBox.vue";
import PanelCard from "../ui/PanelCard.vue";

const presence = usePresenceStore();
onMounted(() => void presence.initialize());

const state = computed(() => presence.status?.state ?? null);
const notice = computed(() =>
  state.value === null ? null : presenceNotice(state.value),
);

const enabled = computed({
  get: () => presence.enabled,
  set: (value: boolean) => void presence.setEnabled(value),
});

/** Onglet de l'aperçu ; suit la phase réelle tant qu'on n'en choisit pas. */
const previewPhase = ref<LinkPhase>("ground");
watch(
  () => presence.status?.phase.phase,
  (phase) => {
    if (phase) previewPhase.value = phase;
  },
  { immediate: true },
);

/** Durée de la phase réelle ; zéro pour l'autre onglet de l'aperçu. */
const since = computed(() => {
  const phase = presence.status?.phase;
  return phase && phase.phase === previewPhase.value ? phase.since : null;
});
const elapsed = useElapsed(since, { clock: () => Date.now(), tickMs: 1_000 });
</script>

<template>
  <PanelCard>
    <template #kicker>Discord Rich Presence</template>
    <template #aside>
      <span v-if="state" class="caps badge" :class="state">
        {{ presenceBadge(state) }}
      </span>
    </template>

    <AppSwitch
      v-model="enabled"
      label="Afficher mon activité sur Discord"
      :disabled="!presence.available"
      :busy-label="
        presence.toggling ? (enabled ? 'Activation' : 'Désactivation') : null
      "
    />
    <p class="intro">
      Atlas indique sur votre profil Discord que vous pilotez un serveur.
      Désactivé par défaut ; aucune donnée ne quitte l'application tant que
      l'interrupteur reste éteint.
    </p>

    <div class="sharing">
      <section class="list">
        <h3 class="caps list-title">Partagé</h3>
        <ul>
          <li><AppIcon name="check" />Le nom : {{ APP_NAME }}</li>
          <li><AppIcon name="check" />Votre état : En liaison ou Au sol</li>
          <li><AppIcon name="check" />La durée écoulée</li>
          <li>
            <AppIcon name="check" />Un bouton « {{ SOURCE_LABEL }} » vers le
            dépôt GitHub d'Atlas
          </li>
        </ul>
      </section>
      <section class="list">
        <h3 class="caps list-title">Jamais partagé</h3>
        <ul>
          <li><AppIcon name="never" />Le nom du profil</li>
          <li><AppIcon name="never" />Le serveur et son adresse</li>
          <li><AppIcon name="never" />Les fichiers et les commandes</li>
        </ul>
      </section>
    </div>

    <NoticeBox v-if="notice" :tone="notice.tone" :title="notice.title">
      {{ notice.text }}
      <template v-if="notice.retry" #action>
        <AppButton size="compact" @click="presence.retry()">
          Réessayer
        </AppButton>
      </template>
    </NoticeBox>

    <div class="preview-head">
      <p class="caps preview-title">Aperçu sur Discord</p>
      <div class="phases" role="group" aria-label="Aperçu de l'état">
        <button
          v-for="phase in ['linked', 'ground'] as const"
          :key="phase"
          type="button"
          class="phase"
          :aria-pressed="previewPhase === phase"
          @click="previewPhase = phase"
        >
          {{ PHASE_LABEL[phase] }}
        </button>
      </div>
    </div>

    <div
      v-if="state"
      class="card"
      :class="{ published: state === 'connected' }"
    >
      <p class="caps card-kicker">{{ previewKicker(state) }}</p>
      <div class="card-body">
        <span class="card-image">
          <img :src="symbol" alt="" draggable="false" />
        </span>
        <div class="card-text">
          <p class="card-name">{{ APP_NAME }}</p>
          <p class="card-state">{{ PHASE_LABEL[previewPhase] }}</p>
          <p class="card-elapsed mono">
            {{ formatClock(elapsed ?? 0) }} écoulé
          </p>
        </div>
      </div>
      <!-- Imitation du bouton de Discord, non cliquable ici. -->
      <span class="card-button" aria-hidden="true">{{ SOURCE_LABEL }}</span>
    </div>
  </PanelCard>
</template>

<style scoped>
.badge.searching {
  color: var(--color-warning);
}

.badge.connected {
  color: var(--color-success);
}

.intro {
  margin: 0;
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.sharing {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--sp-3);
}

.list {
  padding: var(--sp-3) var(--sp-4);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border-strong);
}

.list-title {
  margin: 0 0 var(--sp-2);
  color: var(--color-text-muted);
}

.list ul {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: var(--fs-sm);
}

.list li {
  display: flex;
  align-items: baseline;
  gap: var(--sp-2);
}

.list li :deep(.icon) {
  color: var(--color-text-muted);
}

.list:first-child li :deep(.icon) {
  color: var(--color-success);
}

.preview-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
}

.preview-title {
  margin: 0;
  color: var(--color-text-muted);
}

.phases {
  display: inline-flex;
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-1);
}

.phase {
  padding: var(--sp-1) var(--sp-3);
  background: var(--color-surface);
  border: none;
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
  font-weight: 600;
  cursor: pointer;
}

.phase + .phase {
  border-left: var(--border-width) solid var(--color-border-strong);
}

.phase:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.phase[aria-pressed="true"] {
  background: var(--color-selection);
  box-shadow: inset 0 0 0 var(--border-width) var(--color-action);
  color: var(--color-action);
}

.card {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-4);
  background: var(--color-field);
  border: var(--border-width) dashed var(--color-border-strong);
}

.card.published {
  border-style: solid;
}

.card-kicker {
  margin: 0;
  color: var(--color-text-subtle);
}

.card-body {
  display: flex;
  align-items: center;
  gap: var(--sp-4);
}

.card:not(.published) .card-body {
  opacity: var(--opacity-unpublished);
}

.card-image {
  display: inline-flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  width: var(--discord-image-size);
  height: var(--discord-image-size);
  background: var(--color-bg);
  border: var(--border-width) solid var(--color-border);
}

.card-image img {
  width: calc(var(--discord-image-size) - 2 * var(--sp-3));
}

.card-text {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.card-text p {
  margin: 0;
}

.card-name {
  font-weight: 600;
}

.card-state {
  font-size: var(--fs-sm);
}

.card-elapsed {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.card-button {
  padding: var(--sp-1) var(--sp-3);
  background: var(--color-surface-raised);
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-1);
  font-size: var(--fs-sm);
  font-weight: 600;
  text-align: center;
}

.card:not(.published) .card-button {
  opacity: var(--opacity-unpublished);
}
</style>
