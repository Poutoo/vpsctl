<script setup lang="ts">
/**
 * Panneau « Fond d'écran » des préférences (planche VI) : image utilisée,
 * assombrissement, couleurs d'origine et aperçu de la lisibilité.
 */
import { computed } from "vue";
import { usePreferencesStore } from "../../stores/preferences";
import PanelCard from "../ui/PanelCard.vue";
import RangeSlider from "../ui/RangeSlider.vue";
import DimmingPreview from "./DimmingPreview.vue";

const preferences = usePreferencesStore();

const dimming = computed({
  get: () => preferences.dimming,
  set: (value: number) => preferences.setDimming(value),
});

const originalColors = computed({
  get: () => preferences.originalColors,
  set: (value: boolean) => void preferences.setOriginalColors(value),
});
</script>

<template>
  <PanelCard>
    <template #kicker>Fond d'écran</template>
    <template #aside><span class="caps">Fond d'origine</span></template>

    <div class="current">
      <span class="thumbnail" aria-hidden="true" />
      <div class="identity">
        <p class="name">Fond d'origine</p>
        <p class="meta mono">Fourni avec Atlas</p>
      </div>
    </div>

    <hr class="separator" />

    <RangeSlider
      v-model="dimming"
      label="Assombrissement"
      :value-text="`${preferences.dimming} %`"
      min-hint="0 % · image nette"
      max-hint="100 % · image masquée"
    />

    <label class="option">
      <input v-model="originalColors" type="checkbox" />
      <span class="option-text">
        <span class="option-title">Couleurs d'origine</span>
        <span class="help">
          Affiche l'image en couleurs, sans la désaturation habituelle.
        </span>
      </span>
    </label>

    <div class="live">
      <p class="caps live-title">Aperçu en direct · lisibilité</p>
      <DimmingPreview />
    </div>
  </PanelCard>
</template>

<style scoped>
.current {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-4);
}

.thumbnail {
  flex-shrink: 0;
  width: var(--thumbnail-width);
  height: var(--thumbnail-height);
  background: var(--color-bg) var(--bg-image) center / cover no-repeat;
  border: var(--border-width) solid var(--color-border-strong);
  filter: var(--bg-filter);
}

.identity {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.name {
  margin: 0;
  overflow: hidden;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta {
  margin: 0;
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.separator {
  width: 100%;
  margin: 0;
  border: none;
  border-top: var(--border-width) solid var(--color-border);
}

.option {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-2);
  cursor: pointer;
}

.option input {
  margin-top: var(--sp-1);
}

.option-text {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
}

.option-title {
  font-size: var(--fs-md);
  font-weight: 600;
}

.help {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.live {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}

.live-title {
  margin: 0;
  color: var(--color-text-muted);
}
</style>
