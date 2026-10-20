import { computed, reactive, ref, shallowRef, triggerRef } from "vue";
import { errorMessage } from "../api/errors";
import type { AppError } from "../bindings/AppError";
import type { LogEvent } from "../bindings/LogEvent";
import type { LogLine } from "../bindings/LogLine";

/** Lignes gardées en mémoire : les plus anciennes sont supprimées. */
export const LOG_CAPACITY = 5_000;

/** Ligne de console : une ligne reçue, ou un avis local (lignes écartées). */
export interface ConsoleLine extends LogLine {
  /** Identifiant stable, pour la liste virtualisée. */
  id: number;
  notice: boolean;
}

export type StreamStatus =
  | "idle"
  | "starting"
  | "live"
  /** La commande s'est terminée d'elle-même. */
  | "ended"
  /** Arrêt demandé. */
  | "stopped"
  | "error"
  /** Liaison perdue : issue inconnue, aucune reprise automatique. */
  | "lost";

/** Ligne affichée comme erreur (style rouge) : priorité syslog de 0 à 3. */
export function isJournalError(line: LogLine): boolean {
  return (
    line.origin === "journal" && line.priority !== null && line.priority <= 3
  );
}

/** Ouverture et arrêt d'un flux côté backend (logs ou script). */
export interface StreamDriver {
  /** Ouvre le flux et renvoie son identifiant. */
  open(onEvent: (event: LogEvent) => void): Promise<string>;
  /** Rouvre un flux interrompu à partir de cet instant, sans historique. */
  resume?(onEvent: (event: LogEvent) => void): Promise<string>;
  close(id: string): Promise<void>;
}

/** Heure locale `09:41:07`. */
function clockTime(millis: number): string {
  return new Date(millis).toLocaleTimeString("fr-FR", { hour12: false });
}

export interface LogSessionOptions {
  capacity?: number;
  /**
   * Fin du flux sans arrêt demandé : `ended` (commande terminée d'elle-même)
   * ou `error` (liaison perdue, ouverture refusée).
   */
  onFinish?: (session: LogSession) => void;
}

/**
 * État complet d'un flux de logs ou de la sortie d'un script : ouverture,
 * mémoire tampon limitée, compteurs, arrêt, relance. Indépendante de tout
 * composant : la session (et son `Channel`) vit tant qu'on la garde, quel
 * que soit l'écran affiché. Toutes les sessions appartiennent au store
 * `telemetry`, qui décide quand les arrêter.
 */
export function createLogSession(
  driver: StreamDriver,
  { capacity = LOG_CAPACITY, onFinish }: LogSessionOptions = {},
) {
  const lines = shallowRef<ConsoleLine[]>([]);
  const status = ref<StreamStatus>("idle");
  const exitCode = ref<number | null>(null);
  const error = ref<string | null>(null);
  /** Début de l'ouverture en cours (`performance.now()`), pour le chrono. */
  const startingSince = ref<number | null>(null);
  /** Début du flux en cours ou terminé (horodatage en ms), pour le chrono T+. */
  const startedAt = ref<number | null>(null);
  /** Fin du flux (horodatage en ms), pour la bannière LOS. */
  const endedAt = ref<number | null>(null);
  /** Lignes de la sortie d'erreur, et erreurs du journal, dans la mémoire. */
  const stderrCount = ref(0);
  const errorCount = ref(0);
  /** Lignes ajoutées depuis le dernier effacement (la mémoire, elle, est bornée). */
  const received = ref(0);

  let streamId: string | null = null;
  /** Incrémenté à chaque ouverture ou arrêt : les événements d'un flux remplacé sont ignorés. */
  let generation = 0;
  let nextLineId = 1;

  function count(line: LogLine, delta: number) {
    if (line.origin === "stderr") stderrCount.value += delta;
    if (isJournalError(line)) errorCount.value += delta;
  }

  function append(incoming: ConsoleLine[]) {
    const buffer = lines.value;
    for (const line of incoming) {
      buffer.push(line);
      count(line, 1);
    }
    const excess = buffer.length - capacity;
    if (excess > 0) {
      for (const line of buffer.splice(0, excess)) count(line, -1);
    }
    received.value += incoming.length;
    triggerRef(lines);
  }

  function notice(text: string): ConsoleLine {
    return {
      id: nextLineId++,
      text,
      timestamp: null,
      origin: "stdout",
      priority: null,
      missing: false,
      notice: true,
    };
  }

  function onEvent(current: number, event: LogEvent) {
    if (current !== generation) return;
    switch (event.type) {
      case "lines": {
        const batch = event.lines.map((line): ConsoleLine => ({
          ...line,
          id: nextLineId++,
          notice: false,
        }));
        if (event.dropped > 0) {
          batch.push(
            notice(
              `${event.dropped} ${event.dropped > 1 ? "lignes non affichées" : "ligne non affichée"} : débit trop élevé`,
            ),
          );
        }
        append(batch);
        break;
      }
      case "ended":
        exitCode.value = event.exitCode;
        finish("ended");
        onFinish?.(session);
        break;
      case "error":
        error.value = event.message;
        finish("error");
        onFinish?.(session);
        break;
    }
  }

  function finish(next: StreamStatus) {
    generation++;
    streamId = null;
    status.value = next;
    startingSince.value = null;
    endedAt.value = Date.now();
  }

  /**
   * Ouvre le flux. Renvoie `false` si l'ouverture a échoué (message dans
   * `error`) ou si le flux a été arrêté pendant l'ouverture.
   */
  function start(): Promise<boolean> {
    return open(driver.open);
  }

  async function open(openStream: StreamDriver["open"]): Promise<boolean> {
    const current = ++generation;
    status.value = "starting";
    startingSince.value = performance.now();
    startedAt.value = Date.now();
    exitCode.value = null;
    error.value = null;
    endedAt.value = null;
    try {
      const id = await openStream((event) => onEvent(current, event));
      if (current !== generation) {
        // Terminé, arrêté ou remplacé pendant l'ouverture : un flux encore
        // ouvert n'a plus d'usage (identifiant inconnu ignoré sinon).
        void driver.close(id).catch(() => undefined);
        // Relu après l'attente : l'événement final a pu le changer.
        const reached = status.value as StreamStatus;
        return reached === "ended" || reached === "error";
      }
      streamId = id;
      status.value = "live";
      startingSince.value = null;
      return true;
    } catch (raw) {
      if (current !== generation) return false;
      error.value = errorMessage(raw as AppError);
      finish("error");
      return false;
    }
  }

  /** Arrête le flux en cours (sans effet s'il n'y en a pas). */
  async function stop() {
    const id = streamId;
    const wasActive = status.value === "live" || status.value === "starting";
    if (wasActive) finish("stopped");
    else generation++;
    if (id !== null) await driver.close(id).catch(() => undefined);
  }

  function clear() {
    lines.value = [];
    received.value = 0;
    stderrCount.value = 0;
    errorCount.value = 0;
  }

  /** Rouvre le flux sur la même source (« Rejouer », « Relancer »). */
  async function replay(): Promise<boolean> {
    await stop();
    clear();
    return start();
  }

  /**
   * Liaison perdue : le flux est marqué interrompu, sans appel au backend
   * (qui l'a déjà arrêté) ni `onFinish`. Ses événements tardifs sont ignorés.
   */
  function interrupt() {
    if (status.value === "live" || status.value === "starting") {
      finish("lost");
    }
  }

  /**
   * « Reprendre » après un LOS : le flux repart de cet instant, à la suite
   * des lignes gardées ; ce qui a été émis pendant la coupure n'est pas
   * récupéré, un avis marque le trou.
   */
  function resume(): Promise<boolean> {
    const interruptedAt = endedAt.value;
    const gap =
      interruptedAt === null
        ? `Flux repris à ${clockTime(Date.now())}`
        : `Flux interrompu à ${clockTime(interruptedAt)}, repris à ${clockTime(Date.now())} · lignes de l'intervalle non récupérées`;
    append([notice(gap)]);
    return open(driver.resume ?? driver.open);
  }

  const active = computed(
    () => status.value === "live" || status.value === "starting",
  );

  const session = reactive({
    lines,
    status,
    active,
    exitCode,
    error,
    startingSince,
    startedAt,
    endedAt,
    stderrCount,
    errorCount,
    received,
    start,
    stop,
    clear,
    replay,
    interrupt,
    resume,
  });
  return session;
}

export type LogSession = ReturnType<typeof createLogSession>;
