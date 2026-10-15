<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  /** Empreinte au format `SHA256:…` (43 caractères base64). */
  fingerprint: string;
  /** Empreinte de référence : les groupes différents sont signalés. */
  compareTo?: string | null;
  /** Affiche aussi la chaîne continue, pour la comparer à `ssh-keygen`. */
  showFull?: boolean;
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
  <div class="fingerprint">
    <ol class="groups">
      <li
        v-for="group in groups"
        :key="group.number"
        :class="{ differs: group.differs }"
      >
        <span class="number">{{ group.number }}</span>
        <span class="text">{{ group.text }}</span>
      </li>
    </ol>
    <p v-if="showFull" class="full">{{ fingerprint }}</p>
  </div>
</template>

<style scoped>
.groups {
  display: grid;
  grid-template-columns: repeat(4, max-content);
  gap: var(--sp-3) var(--sp-5);
  margin: 0;
  padding: 0;
  list-style: none;
}

.groups li {
  display: flex;
  flex-direction: column;
}

.number {
  color: var(--color-text-muted);
  font-size: var(--fs-md);
}

.text {
  font-family: var(--font-mono);
  font-size: var(--fs-xl);
}

.differs .text {
  color: var(--color-danger-text);
  text-decoration: underline;
}

.full {
  margin: var(--sp-4) 0 0;
  font-family: var(--font-mono);
  font-size: var(--fs-md);
  user-select: all;
  word-break: break-all;
}
</style>
