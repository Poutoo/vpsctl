<script setup lang="ts">
/**
 * Vue Différence (lecture seule) : version du serveur (−) contre la vôtre
 * (+), chaque changement attribué au serveur ou à vous d'après la version
 * d'ouverture. Teinte ambre pour la version du serveur, vert-de-gris pour
 * la vôtre ; les signes et l'étiquette doublent la couleur. Une seule
 * action par bloc du serveur ; celle d'un bloc mixte annonce les lignes à
 * vous qu'elle efface.
 */
import { computed } from "vue";
import type { BlockAction, DiffRow } from "../../editorDiff";
import { buildDiff } from "../../editorDiff";
import { timeLabel } from "../../linkText";
import AppButton from "../ui/AppButton.vue";

const props = defineProps<{
  base: string;
  server: string;
  yours: string;
  /** Modification de la version du serveur (ms epoch). */
  serverModified: number | null;
  /** Enregistrement impossible (liaison perdue, enregistrement en cours). */
  saveDisabled: boolean;
}>();

const emit = defineEmits<{
  back: [];
  save: [];
  apply: [action: BlockAction];
}>();

const view = computed(() => buildDiff(props.base, props.server, props.yours));

const serverLeft = computed(
  () => view.value.blocks.filter((b) => b.kind !== "yours").length > 0,
);

function sign(row: DiffRow): string {
  return row.type === "removed" ? "−" : row.type === "added" ? "+" : "";
}

function tag(row: DiffRow): string | null {
  if (row.type === "same") return null;
  return row.who === "server" ? "Serveur" : "Vous";
}

function gapLabel(count: number): string {
  return count > 1 ? `… ${count} lignes identiques` : "… 1 ligne identique";
}
</script>

<template>
  <section class="diff" aria-label="Différence" data-testid="editor-diff">
    <header class="bar">
      <span class="caps title"
        >Différence · Serveur → Vous · Lecture seule</span
      >
      <AppButton size="compact" @click="emit('back')">
        ← Retour à l'édition
      </AppButton>
      <AppButton
        size="compact"
        variant="primary"
        :disabled="saveDisabled"
        @click="emit('save')"
      >
        Enregistrer…
      </AppButton>
    </header>
    <p class="legend">
      <span
        ><span class="mono sign">−</span> version du serveur<template
          v-if="serverModified !== null"
        >
          · modifiée à {{ timeLabel(serverModified) }}</template
        ></span
      >
      <span><span class="mono sign">+</span> votre version</span>
      <span
        ><span class="caps who server">Serveur</span> changement fait sur le
        serveur, perdu si vous écrasez</span
      >
    </p>
    <div class="rows" role="table" aria-label="Lignes comparées">
      <p v-if="view.blocks.length === 0" class="empty">
        Votre version et celle du serveur sont identiques.
      </p>
      <p v-else-if="!serverLeft" class="empty">
        Tous les changements du serveur sont repris : écraser ne perd plus rien.
      </p>
      <template v-for="(item, index) in view.items" :key="index">
        <div v-if="item.type === 'gap'" class="gap mono" role="row">
          {{ gapLabel(item.count) }}
        </div>
        <div v-else-if="item.type === 'row'" class="row" role="row">
          <span class="num mono">{{
            item.row.type === "same" ? item.row.server : ""
          }}</span>
          <span class="num mono">{{
            item.row.type === "same" ? item.row.yours : ""
          }}</span>
          <span class="mono sign" />
          <span class="mono text">{{ item.row.text }}</span>
        </div>
        <div
          v-else
          class="block"
          :class="item.block.kind"
          :data-testid="`diff-block-${item.block.kind}`"
        >
          <div
            v-for="(row, rowIndex) in item.block.rows"
            :key="rowIndex"
            class="row"
            :class="row.type"
            role="row"
          >
            <span class="num mono">{{
              row.type === "added" ? "" : row.server
            }}</span>
            <span class="num mono">{{
              row.type === "removed" ? "" : row.yours
            }}</span>
            <span class="mono sign">{{ sign(row) }}</span>
            <span class="mono text">{{ row.text }}</span>
            <span
              v-if="tag(row)"
              class="caps who"
              :class="row.type !== 'same' && row.who"
              >{{ tag(row) }}</span
            >
            <AppButton
              v-if="
                rowIndex === 0 &&
                item.block.action &&
                item.block.action.kind !== 'takeMixed'
              "
              size="compact"
              class="action"
              :data-testid="`diff-action-${item.block.action.kind}`"
              @click="emit('apply', item.block.action)"
            >
              {{ item.block.action.label }}
            </AppButton>
          </div>
          <div
            v-if="item.block.action?.kind === 'takeMixed'"
            class="mixed-action"
          >
            <span class="caps who mixed">Vous et le serveur</span>
            <AppButton
              size="compact"
              variant="danger"
              data-testid="diff-action-takeMixed"
              @click="emit('apply', item.block.action)"
            >
              {{ item.block.action.label }}
            </AppButton>
          </div>
        </div>
      </template>
    </div>
  </section>
</template>

<style scoped>
.diff {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  background: var(--color-field);
}

.bar {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  padding: var(--sp-2) var(--sp-4);
  background: var(--color-surface-raised);
  border-bottom: var(--border-width) solid var(--color-border);
}

.title {
  margin-right: auto;
  color: var(--color-action);
}

.legend {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-5);
  margin: 0;
  padding: var(--sp-2) var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.rows {
  flex: 1;
  min-height: 0;
  overflow: auto;
  font-size: var(--fs-data);
}

.empty {
  margin: 0;
  padding: var(--sp-3) var(--sp-4);
  color: var(--color-text-muted);
  font-size: var(--fs-md);
}

.gap {
  padding: var(--sp-1) var(--sp-4) var(--sp-1)
    calc(2 * var(--editor-gutter-width));
  background: var(--color-surface);
  border-top: var(--border-width) solid var(--color-border);
  border-bottom: var(--border-width) solid var(--color-border);
  color: var(--color-text-muted);
}

.row {
  display: grid;
  grid-template-columns:
    var(--editor-gutter-width) var(--editor-gutter-width) var(--sp-5)
    minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 0 var(--sp-2);
  min-height: var(--editor-line-height);
  padding-right: var(--sp-3);
  line-height: var(--editor-line-height);
}

.num {
  padding-right: var(--sp-2);
  color: var(--color-text-subtle);
  text-align: right;
}

.sign {
  text-align: center;
}

.text {
  overflow: hidden;
  white-space: pre;
  text-overflow: ellipsis;
}

/* Version du serveur : ambre ; la vôtre : vert-de-gris. */
.row.removed {
  background: var(--color-diff-server);
}

.row.removed .sign {
  color: var(--color-action);
}

.row.added {
  background: var(--color-diff-yours);
}

.row.added .sign {
  color: var(--color-success);
}

.who {
  color: var(--color-text-muted);
}

.who.server {
  color: var(--color-action);
}

.action {
  margin: var(--sp-1) 0;
}

.mixed-action {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--sp-3);
  padding: var(--sp-1) var(--sp-3);
  background: var(--color-surface);
  border-bottom: var(--border-width) solid var(--color-border);
}

.who.mixed {
  color: var(--color-warning);
}
</style>
