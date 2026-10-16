import { reactive } from "vue";

/** Durée d'affichage d'une notification de succès. */
const SUCCESS_MS = 5_000;
/** Notifications à l'écran au maximum : la plus ancienne laisse sa place. */
const MAX_VISIBLE = 3;

export interface NotifyAction {
  label: string;
  run: () => void;
}

export interface NotifyOptions {
  tone: "success" | "error";
  title: string;
  /** Objet et complément en monospace, ex. « bot.service · 0,8 s ». */
  detail?: string;
  /** Lien d'action, ex. « Voir la télémétrie ». */
  action?: NotifyAction;
}

export interface Notification extends NotifyOptions {
  id: number;
}

const items = reactive<Notification[]>([]);
const timers = new Map<number, ReturnType<typeof setTimeout>>();
let nextId = 1;

function dismiss(id: number) {
  const index = items.findIndex((item) => item.id === id);
  if (index >= 0) items.splice(index, 1);
  const timer = timers.get(id);
  if (timer !== undefined) clearTimeout(timer);
  timers.delete(id);
}

/**
 * Notifications de l'application (`ToastHost`, monté une seule fois dans
 * `App.vue`) : succès fermé après 5 s, erreur persistante jusqu'à fermeture,
 * trois au maximum.
 */
export function useNotify() {
  function notify(options: NotifyOptions): number {
    const id = nextId++;
    items.push({ ...options, id });
    while (items.length > MAX_VISIBLE) dismiss(items[0].id);
    if (options.tone === "success") {
      timers.set(
        id,
        setTimeout(() => dismiss(id), SUCCESS_MS),
      );
    }
    return id;
  }

  return { items, notify, dismiss };
}
