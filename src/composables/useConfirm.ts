import { reactive } from "vue";

export interface ConfirmCheck {
  label: string;
  /** Donnée brute en monospace, ex. « Exited (137) 4 days ago ». */
  detail?: string;
}

export interface ConfirmOptions {
  title: string;
  /** Objet de l'action (nom, chemin), affiché sous le titre. */
  subject?: string;
  /** Conditions vérifiées, listées avant les conséquences (ex. « Conteneur arrêté »). */
  checks?: ConfirmCheck[];
  /** Conséquences de l'action, listées dans la fenêtre. */
  consequences: string[];
  /** Libellé du bouton GO, ex. « Supprimer ». */
  confirmLabel: string;
  /** Action destructrice : signalée comme telle dans la modale. */
  danger?: boolean;
  /** Case à cocher qui doit l'être pour activer GO. */
  acknowledge?: string;
  /** Avertissement mis en évidence avant la case à cocher. */
  warning?: string;
}

interface ConfirmState extends ConfirmOptions {
  open: boolean;
  resolve: ((confirmed: boolean) => void) | null;
}

const state = reactive<ConfirmState>({
  open: false,
  title: "",
  subject: undefined,
  checks: [],
  consequences: [],
  confirmLabel: "",
  danger: false,
  acknowledge: undefined,
  warning: undefined,
  resolve: null,
});

/**
 * Confirmation explicite GO / NO GO avant une action destructrice. La modale
 * (`ConfirmDialog`) est montée une seule fois dans `App.vue`.
 */
export function useConfirm() {
  function confirm(options: ConfirmOptions): Promise<boolean> {
    state.resolve?.(false);
    Object.assign(state, {
      subject: undefined,
      checks: [],
      danger: false,
      acknowledge: undefined,
      warning: undefined,
      ...options,
      open: true,
    });
    return new Promise((resolve) => {
      state.resolve = resolve;
    });
  }

  function settle(confirmed: boolean) {
    state.open = false;
    state.resolve?.(confirmed);
    state.resolve = null;
  }

  return { state, confirm, settle };
}
