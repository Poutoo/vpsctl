<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  /** Empreinte au format `SHA256:…` (43 caractères base64). */
  fingerprint: string;
  /** Empreinte de référence : les groupes différents sont signalés. */
  compareTo?: string | null;
  /** Affiche aussi la chaîne continue, pour la comparer à `ssh-keygen`. */
  showFull?: boolean;
  /** Grille numérotée (comparaison groupe par groupe) ou ligne continue. */
  layout?: "grid" | "inline";
}>();

const PREFIX = "SHA256:";
const GROUP_SIZE = 4;

function groupsOf(fingerprint: string): string[] {
  const body = fingerprint.startsWith(PREFIX)
    ? fingerprint.slice(PREFIX.length)
    : fingerprint;
  const groups: string[] = [];
  for (let i = 0; i < body.length; i += GROUP_SIZE) {
    groups.push(body.slice(i, i + GROUP_SIZE));
  }
  return groups;
}

const groups = computed(() => {
  const reference = props.compareTo ? groupsOf(props.compareTo) : null;
  return groupsOf(props.fingerprint).map((text, index) => ({
    number: String(index + 1).padStart(2, "0"),
    text,
    differs: reference !== null && reference[index] !== text,
  }));
});
</script>

<template>
  <div class="fingerprint" :class="layout ?? 'grid'">
    <ol class="groups">
      <li
        v-for="group in groups"
        :key="group.number"
        :class="{ differs: group.differs }"
      >
        <span class="caps number">{{ group.number }}</span>
        <span class="text">{{ group.text }}</span>
        <span v-if="group.differs" class="visually-hidden">(différent)</span>
      </li>
    </ol>
    <p v-if="showFull" class="full">{{ fingerprint }}</p>
  </div>
</template>

<style scoped>
.groups {
  margin: 0;
  padding: 0;
  list-style: none;
  font-family: var(--font-mono);
}

/* Grille : quatre groupes par ligne, numérotés. */
.grid .groups {
  display: grid;
  grid-template-columns: repeat(4, max-content);
  gap: var(--sp-3) var(--sp-5);
}

.grid li {
  display: flex;
  flex-direction: column;
}

.number {
  color: var(--color-text-muted);
}

.grid .text {
  font-size: var(--fs-xl);
}

/* Ligne : groupes séparés par une espace, sans numéros. */
.inline .groups {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-1) var(--sp-3);
  font-size: var(--fs-lg);
}

.inline .number {
  display: none;
}

/* Différence : soulignement (forme) et texte rouge lisible. */
.differs .text {
  color: var(--color-danger-text);
  text-decoration: underline;
  text-underline-offset: var(--sp-1);
}

.full {
  margin-top: var(--sp-3);
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  user-select: all;
  word-break: break-all;
}
</style>
