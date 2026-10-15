import { reactive } from "vue";

export interface ConfirmOptions {
  title: string;
  message: string;
  confirmLabel: string;
  /** Action destructrice : signalée comme telle dans la modale. */
  danger?: boolean;
}

interface ConfirmState extends ConfirmOptions {
  open: boolean;
  resolve: ((confirmed: boolean) => void) | null;
}

const state = reactive<ConfirmState>({
  open: false,
  title: "",
  message: "",
  confirmLabel: "",
  danger: false,
  resolve: null,
});

/**
 * Confirmation explicite avant une action destructrice. La modale
 * (`ConfirmDialog`) est montée une seule fois dans `App.vue`.
 */
export function useConfirm() {
  function confirm(options: ConfirmOptions): Promise<boolean> {
    state.resolve?.(false);
    Object.assign(state, { danger: false, ...options, open: true });
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
