<script setup lang="ts">
/**
 * Menu d'une entrée de l'explorateur, identique en menu contextuel (clic
 * droit) et en menu déroulant (bouton « ⋯ », touche Menu) : Ouvrir
 * (dossiers), Télécharger (fichiers), Renommer, Copier le chemin, et
 * Supprimer isolé en bas, en rouge, qui ouvre toujours un GO / NO GO.
 *
 * À placer dans un `ContextMenuRoot` ou un `DropdownMenuRoot` (Reka UI),
 * selon `variant`. Comportements (focus, flèches, Échap) de Reka UI.
 */
import {
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuLabel,
  ContextMenuPortal,
  ContextMenuSeparator,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuSeparator,
} from "reka-ui";
import { computed } from "vue";
import type { FileEntry } from "../../bindings/FileEntry";
import { isDirLike, isFileLike } from "../../remoteFiles";

export type FileAction = "open" | "download" | "rename" | "copyPath" | "delete";

const props = defineProps<{
  entry: FileEntry;
  variant: "context" | "dropdown";
  /**
   * Menu déroulant : élément qui l'ancre. Nécessaire quand le déclencheur est
   * enveloppé dans une infobulle, dont le contexte de positionnement capte
   * l'ancre du menu.
   */
  reference?: HTMLElement | null;
}>();

const emit = defineEmits<{
  action: [action: FileAction];
  /** Fermeture : le focus est rendu par le parent (événement annulable). */
  closeAutoFocus: [event: Event];
}>();

const parts = computed(() =>
  props.variant === "context"
    ? {
        Portal: ContextMenuPortal,
        Content: ContextMenuContent,
        Item: ContextMenuItem,
        Label: ContextMenuLabel,
        Separator: ContextMenuSeparator,
      }
    : {
        Portal: DropdownMenuPortal,
        Content: DropdownMenuContent,
        Item: DropdownMenuItem,
        Label: DropdownMenuLabel,
        Separator: DropdownMenuSeparator,
      },
);

/**
 * Placement du menu déroulant ; le menu contextuel s'ouvre au point de clic.
 * `reference` reste présente même à `null` (Reka UI ne suit pas une prop
 * absente au montage) ; à `null`, Reka UI revient à l'ancre de son contexte.
 */
const placement = computed(() =>
  props.variant === "dropdown"
    ? {
        side: "bottom" as const,
        align: "end" as const,
        sideOffset: 4,
        // `null` accepté à l'exécution (prop sans type, transmise si elle n'est
        // pas `undefined`), mais absent des types de Reka UI.
        reference: (props.reference ?? null) as unknown as
          HTMLElement | undefined,
      }
    : {},
);

const items = computed(() => {
  const list: { action: FileAction; label: string; shortcut: string }[] = [];
  if (isDirLike(props.entry)) {
    list.push({ action: "open", label: "Ouvrir", shortcut: "↵" });
  }
  if (isFileLike(props.entry)) {
    list.push({ action: "download", label: "Télécharger", shortcut: "Ctrl D" });
  }
  list.push(
    { action: "rename", label: "Renommer", shortcut: "F2" },
    { action: "copyPath", label: "Copier le chemin", shortcut: "Ctrl ⇧ C" },
  );
  return list;
});
</script>

<template>
  <component :is="parts.Portal">
    <component
      :is="parts.Content"
      class="app-menu"
      v-bind="placement"
      @close-auto-focus="(event: Event) => emit('closeAutoFocus', event)"
    >
      <component :is="parts.Label" class="app-menu-label">{{
        entry.path
      }}</component>
      <component
        :is="parts.Item"
        v-for="item in items"
        :key="item.action"
        class="app-menu-item"
        @select="emit('action', item.action)"
      >
        <span>{{ item.label }}</span>
        <kbd class="app-menu-shortcut">{{ item.shortcut }}</kbd>
      </component>
      <component :is="parts.Separator" class="app-menu-separator" />
      <component
        :is="parts.Item"
        class="app-menu-item danger"
        @select="emit('action', 'delete')"
      >
        <span>Supprimer</span>
        <kbd class="app-menu-shortcut">Suppr</kbd>
      </component>
    </component>
  </component>
</template>

<!--
  Style non scoped : Reka UI rend le menu dans un portail, hors de la portée
  du composant (même raison que pour les infobulles). Classes préfixées.
-->
<style>
.app-menu {
  z-index: var(--z-menu);
  min-width: calc(4 * var(--sp-8));
  max-width: calc(7 * var(--sp-8));
  padding: var(--sp-1) 0;
  background: var(--color-surface-raised);
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-2);
  box-shadow: var(--shadow-overlay);
  font-family: var(--font-ui);
  font-size: var(--fs-md);
  animation: app-menu-in var(--dur-fast) var(--ease-out);
}

/* Apparition : opacité et translation de 4 px depuis le point d'ouverture. */
@keyframes app-menu-in {
  from {
    opacity: 0;
    transform: translateY(calc(-1 * var(--sp-1)));
  }
}

.app-menu-label {
  padding: var(--sp-2) var(--sp-3);
  overflow: hidden;
  border-bottom: var(--border-width) solid var(--color-border);
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-menu-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-5);
  padding: var(--sp-2) var(--sp-3);
  color: var(--color-text);
  font-weight: 600;
  cursor: pointer;
  outline: none;
  user-select: none;
}

/* Survol ou flèches : fond --surface-3 et trait d'action. */
.app-menu-item[data-highlighted] {
  background: var(--color-surface-hover);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-action);
}

.app-menu-item.danger {
  color: var(--color-danger-text);
}

/* Entrée indisponible : texte atténué, pas de surbrillance. */
.app-menu-item[data-disabled] {
  color: var(--color-text-subtle);
  cursor: not-allowed;
}

.app-menu-item.danger[data-highlighted] {
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-danger);
}

.app-menu-shortcut {
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
  font-weight: 400;
}

.app-menu-separator {
  height: var(--border-width);
  margin: var(--sp-1) 0;
  background: var(--color-border);
}
</style>
