<script setup lang="ts">
/**
 * Écran de refus de l'éditeur (maquettes 12 à 14) : raison, faits constatés,
 * téléchargement proposé sauf pour une clé privée ou un lien. Le contenu
 * n'est jamais affiché.
 */
import { computed } from "vue";
import type { RefusedFile } from "../../bindings/RefusedFile";
import { ownerLabel } from "../../editorFacts";
import { formatBytes, formatModified } from "../../format";
import AppButton from "../ui/AppButton.vue";
import AppIcon from "../ui/AppIcon.vue";

const props = defineProps<{ refused: RefusedFile }>();
const emit = defineEmits<{ download: []; back: [] }>();

interface Fact {
  label: string;
  value: string;
  alert?: boolean;
}

const content = computed(() => {
  const { refusal, file } = props.refused;
  const size = formatBytes(file.size);
  const modified =
    file.modified === null ? "inconnue" : formatModified(file.modified);
  const rights = `${file.permissions} ${file.mode ?? ""} · ${ownerLabel(file.owner)}`;
  const common: Fact[] = [
    { label: "Modifié", value: modified },
    { label: "Droits", value: rights },
  ];
  switch (refusal.reason) {
    case "tooLarge":
      return {
        tag: "Trop volumineux",
        title: "Ce fichier dépasse la limite de l'éditeur.",
        text: `${file.name} pèse ${formatBytes(refusal.size)} ; l'éditeur ouvre les fichiers texte jusqu'à 1 Mo. Téléchargez-le pour le lire sur ce poste, ou suivez-le en direct avec tail -f dans un script.`,
        facts: [
          { label: "Taille", value: formatBytes(refusal.size), alert: true },
          { label: "Limite", value: formatBytes(refusal.limit) },
          ...common,
        ],
      };
    case "binary": {
      const hex = refusal.offset.toString(16).padStart(4, "0");
      return {
        tag: "Contenu binaire",
        title: "Ce fichier n'est pas du texte.",
        text: `Les premiers octets de ${file.name} contiennent des octets nuls. L'éditeur ne modifie que de l'UTF-8 ; modifier ce fichier ici le corromprait.`,
        facts: [
          {
            label: "Détecté",
            value: `premier octet nul à la position ${refusal.offset} (0x${hex})`,
            alert: true,
          },
          { label: "Taille", value: size },
          ...common,
        ],
      };
    }
    case "invalidUtf8":
      return {
        tag: "Encodage",
        title: "Ce fichier n'est pas de l'UTF-8 valide.",
        text: `${file.name} contient des octets qui ne forment pas de l'UTF-8 (autre encodage ou fichier abîmé). L'éditeur ne modifie que de l'UTF-8 ; l'enregistrer d'ici le corromprait.`,
        facts: [
          {
            label: "Détecté",
            value: `première séquence invalide à l'octet ${refusal.offset.toLocaleString("fr-FR")} (ligne ${refusal.line.toLocaleString("fr-FR")})`,
            alert: true,
          },
          { label: "Taille", value: size },
          ...common,
        ],
      };
    case "privateKey":
      return {
        tag: "Clé privée",
        title: "Une clé privée ne s'ouvre jamais dans l'éditeur.",
        text: `${file.name} contient « ${refusal.header} ». Atlas ne la charge pas et ne propose pas de la télécharger d'ici : une clé privée ne doit pas transiter par cette fenêtre.`,
        facts: [
          {
            label: "Détecté",
            value: `en-tête PRIVATE KEY à la ligne ${refusal.line}`,
            alert: true,
          },
          {
            label: "Dossier",
            value:
              props.refused.sshFile === null
                ? file.dir
                : "~/.ssh · fichiers sensibles",
          },
          ...common,
        ],
      };
    case "symlink":
      return {
        tag: "Lien symbolique",
        title: "Un lien symbolique ne s'ouvre pas dans l'éditeur.",
        text: `${file.name} est un lien${refusal.target ? ` vers ${refusal.target}` : ""}. Comme l'explorateur, l'éditeur ne suit jamais les liens : ouvrez la cible elle-même.`,
        facts: [
          {
            label: "Cible",
            value: refusal.target ?? "illisible",
            alert: true,
          },
          { label: "Modifié", value: modified },
        ],
      };
    case "notAFile":
      return {
        tag: "Pas un fichier",
        title: "Cet élément n'est pas un fichier.",
        text: `${file.name} est un dossier ou un fichier spécial (socket, périphérique) : il ne s'ouvre pas dans l'éditeur.`,
        facts: common,
      };
  }
  return null;
});
</script>

<template>
  <div v-if="content" class="refusal" data-testid="editor-refusal">
    <p class="caps tag"><AppIcon name="never" /> Refusé · {{ content.tag }}</p>
    <h2 class="title">{{ content.title }}</h2>
    <p class="text">{{ content.text }}</p>
    <dl class="facts">
      <template v-for="fact in content.facts" :key="fact.label">
        <dt>{{ fact.label }}</dt>
        <dd :class="{ alert: fact.alert }">{{ fact.value }}</dd>
      </template>
    </dl>
    <div class="buttons">
      <AppButton
        v-if="refused.downloadable"
        variant="primary"
        @click="emit('download')"
      >
        <AppIcon name="download" />
        Télécharger {{ formatBytes(refused.file.size) }}
      </AppButton>
      <AppButton @click="emit('back')">Retour aux fichiers</AppButton>
    </div>
  </div>
</template>

<style scoped>
.refusal {
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--sp-4);
  min-height: 0;
  overflow: auto;
  padding: var(--sp-6) var(--sp-6);
  background: var(--color-field);
}

.tag {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  margin: 0;
  color: var(--color-danger-text);
}

.title {
  margin: 0;
  font-size: var(--fs-2xl);
}

.text {
  max-width: var(--editor-text-width);
  margin: 0;
  color: var(--color-text-muted);
  font-size: var(--fs-md);
}

.facts {
  display: grid;
  grid-template-columns: auto auto;
  gap: var(--sp-2) var(--sp-5);
  margin: 0;
  padding: var(--sp-3) var(--sp-4);
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

.facts dt {
  color: var(--color-text-muted);
}

.facts dd {
  margin: 0;
  overflow-wrap: anywhere;
}

.facts dd.alert {
  color: var(--color-danger-text);
}

.buttons {
  display: flex;
  gap: var(--sp-3);
}
</style>
