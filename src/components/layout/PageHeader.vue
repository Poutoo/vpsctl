<script setup lang="ts">
/**
 * En-tête de page : surtitre numéroté (« I · État de la station »), titre et
 * actions à droite. Le surtitre vient de la section, sauf `kicker` explicite.
 */
import { computed } from "vue";
import { SECTIONS, type AppSection } from "../../navigation";

const props = defineProps<{
  title: string;
  section?: AppSection["route"];
  /** Surtitre libre, pour un écran hors navigation. */
  kicker?: string;
  /** Identifiant du titre, pour `aria-labelledby`. */
  titleId?: string;
}>();

const surtitle = computed(() => {
  if (props.kicker) return props.kicker;
  const section = SECTIONS.find((s) => s.route === props.section);
  return section ? `${section.numeral} · ${section.kicker}` : null;
});
</script>

<template>
  <header class="page-header">
    <div class="heading">
      <p v-if="surtitle" class="caps surtitle">{{ surtitle }}</p>
      <h1 :id="titleId">{{ title }}</h1>
    </div>
    <div v-if="$slots.default" class="actions"><slot /></div>
  </header>
</template>

<style scoped>
.page-header {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  justify-content: space-between;
  gap: var(--sp-4);
}

.heading {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.surtitle {
  color: var(--color-text-muted);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-3);
}
</style>
