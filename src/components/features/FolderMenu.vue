<script setup lang="ts">
/**
 * Menu contextuel du dossier affiché, au clic droit dans une zone vide de la
 * liste (ou sur l'état « Dossier vide ») : Nouveau dossier, Nouveau fichier,
 * Téléverser. À placer dans un `ContextMenuRoot` (Reka UI).
 *
 * Styles : classes `app-menu*`, définies (non scopées) dans `FileMenu.vue`.
 */
import {
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuLabel,
  ContextMenuPortal,
  ContextMenuSeparator,
} from "reka-ui";

export type FolderAction = "createDir" | "createFile" | "upload";

defineProps<{
  /** Dossier affiché, en tête du menu. */
  path: string;
  /** Le dossier accepte une création (sinon les entrées sont désactivées). */
  canCreate: boolean;
}>();

const emit = defineEmits<{
  action: [action: FolderAction];
  /** Fermeture : le focus est rendu par le parent (événement annulable). */
  closeAutoFocus: [event: Event];
}>();

const ITEMS: { action: FolderAction; label: string }[] = [
  { action: "createDir", label: "Nouveau dossier" },
  { action: "createFile", label: "Nouveau fichier" },
];
</script>

<template>
  <ContextMenuPortal>
    <ContextMenuContent
      class="app-menu"
      @close-auto-focus="(event: Event) => emit('closeAutoFocus', event)"
    >
      <ContextMenuLabel class="app-menu-label">{{ path }}</ContextMenuLabel>
      <ContextMenuItem
        v-for="item in ITEMS"
        :key="item.action"
        class="app-menu-item"
        :disabled="!canCreate"
        @select="emit('action', item.action)"
      >
        <span>{{ item.label }}</span>
      </ContextMenuItem>
      <ContextMenuSeparator class="app-menu-separator" />
      <ContextMenuItem
        class="app-menu-item"
        :disabled="!canCreate"
        @select="emit('action', 'upload')"
      >
        <span>Téléverser…</span>
      </ContextMenuItem>
    </ContextMenuContent>
  </ContextMenuPortal>
</template>
