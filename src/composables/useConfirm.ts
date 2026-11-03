import { reactive } from "vue";

export interface ConfirmCheck {
  label: string;
  /** Donnée brute en monospace, ex. « Exited (137) 4 days ago ». */
  detail?: string;
}

/** Conséquence qui cite du code (une commande, éventuellement sur plusieurs lignes). */
export interface ConfirmCode {
  label: string;
  code: string;
}

/**
 * Constat listé avec sa forme : coche (`ok`), « ! » (`warn`), triangle
 * (`danger`) ou tiret (`neutral`). Le détail s'affiche en dessous.
 */
export interface ConfirmItem {
  tone: "ok" | "warn" | "danger" | "neutral";
  label: string;
  detail?: string;
}

export interface ConfirmOptions {
  title: string;
  /** Surtitre du bandeau, à la place de « Confirmation requise ». */
  kicker?: string;
  /** Objet de l'action (nom, chemin), affiché sous le titre. */
  subject?: string;
  /** Commande exacte, affichée sous l'objet. */
  command?: string;
  /** Mention à droite du bandeau, à la place de « Irréversible ». */
  aside?: string;
  /** Encadré d'alerte, avant la liste. */
  alert?: { label: string; text: string };
  /** Ce qui est supprimé, listé en premier, avec une marque d'avertissement. */
  removals?: ConfirmCheck[];
  /** Conditions vérifiées, listées avant les conséquences (ex. « Conteneur arrêté »). */
  checks?: ConfirmCheck[];
  /** Constats avec leur forme, listés après les conditions. */
  items?: ConfirmItem[];
  /** Conséquences de l'action, listées dans la fenêtre. */
  consequences: (string | ConfirmCode)[];
  /** Lien sous la liste (choix « link » de `choose`). */
  link?: string;
  /** Libellé du bouton GO, ex. « Supprimer ». */
  confirmLabel: string;
  /** Libellé du bouton NO GO (« Annuler » par défaut). */
  cancelLabel?: string;
  /** Second choix entre NO GO et GO (choix « alternative » de `choose`). */
  alternative?: string;
  /** Action destructrice : signalée comme telle dans la modale. */
  danger?: boolean;
  /** Case à cocher qui doit l'être pour activer GO. */
  acknowledge?: string;
  /** Précision sous la case à cocher. */
  acknowledgeHint?: string;
  /** Case encadrée de rouge (risque de perte d'accès). */
  acknowledgeDanger?: boolean;
  /** Avertissement mis en évidence avant la case à cocher. */
  warning?: string;
}

/** Réponse d'une fenêtre à plusieurs choix. Échap vaut `nogo`. */
export type ConfirmChoice = "go" | "nogo" | "alternative" | "link";

interface ConfirmState extends ConfirmOptions {
  open: boolean;
  resolve: ((choice: ConfirmChoice) => void) | null;
}

const defaults = {
  kicker: undefined,
  subject: undefined,
  command: undefined,
  aside: undefined,
  alert: undefined,
  removals: [],
  checks: [],
  items: [],
  link: undefined,
  cancelLabel: undefined,
  alternative: undefined,
  danger: false,
  acknowledge: undefined,
  acknowledgeHint: undefined,
  acknowledgeDanger: false,
  warning: undefined,
} satisfies Partial<ConfirmOptions>;

const state = reactive<ConfirmState>({
  ...defaults,
  open: false,
  title: "",
  consequences: [],
  confirmLabel: "",
  resolve: null,
});

/**
 * Confirmation explicite GO / NO GO avant une action destructrice. La modale
 * (`ConfirmDialog`) est montée une seule fois dans `App.vue`.
 */
export function useConfirm() {
  /** Fenêtre à plusieurs choix (GO, NO GO, second choix, lien). */
  function choose(options: ConfirmOptions): Promise<ConfirmChoice> {
    state.resolve?.("nogo");
    Object.assign(state, { ...defaults, ...options, open: true });
    return new Promise((resolve) => {
      state.resolve = resolve;
    });
  }

  async function confirm(options: ConfirmOptions): Promise<boolean> {
    return (await choose(options)) === "go";
  }

  function settle(choice: ConfirmChoice | boolean) {
    state.open = false;
    const value =
      typeof choice === "boolean" ? (choice ? "go" : "nogo") : choice;
    state.resolve?.(value);
    state.resolve = null;
  }

  return { state, confirm, choose, settle };
}
