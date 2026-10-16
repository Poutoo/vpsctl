import {
  computed,
  onBeforeUnmount,
  ref,
  shallowRef,
  triggerRef,
  watch,
  type Ref,
} from "vue";
import { errorMessage } from "../api/errors";
import * as logs from "../api/logs";
import type { AppError } from "../bindings/AppError";
import type { LogEvent } from "../bindings/LogEvent";
import type { LogLine } from "../bindings/LogLine";
import type { LogSource } from "../bindings/LogSource";

/** Lignes gardées en mémoire : les plus anciennes sont supprimées. */
export const LOG_CAPACITY = 5_000;
/** Lignes d'historique demandées à l'ouverture du flux. */
const DEFAULT_TAIL = 200;

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
  | "error";

/** Ligne affichée comme erreur (style rouge) : priorité syslog de 0 à 3. */
export function isJournalError(line: LogLine): boolean {
  return (
    line.origin === "journal" && line.priority !== null && line.priority <= 3
  );
}

/**
 * Cycle de vie complet d'un flux de logs (ou, plus tard, de la sortie d'un
 * script) : ouverture, mémoire tampon limitée, arrêt. Le flux est arrêté
 * quand la source change et au démontage du composant : aucun flux ne reste
 * ouvert après un changement d'écran.
 *
 * `source` à `null` : aucun flux.
 */
export function useLogStream(
  source: Readonly<Ref<LogSource | null>>,
  { tail = DEFAULT_TAIL, capacity = LOG_CAPACITY } = {},
) {
  const lines = shallowRef<ConsoleLine[]>([]);
  const status = ref<StreamStatus>("idle");
  const exitCode = ref<number | null>(null);
  const error = ref<string | null>(null);
  /** Début de l'ouverture en cours (`performance.now()`), pour le chrono. */
  const startingSince = ref<number | null>(null);
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
  let unmounted = false;

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
        finish("ended");
        exitCode.value = event.exitCode;
        break;
      case "error":
        finish("error");
        error.value = event.message;
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

  async function start() {
    const target = source.value;
    if (target === null || unmounted) return;
    const current = ++generation;
    status.value = "starting";
    startingSince.value = performance.now();
    exitCode.value = null;
    error.value = null;
    endedAt.value = null;
    try {
      const id = await logs.startLogs(target, tail, (event) =>
        onEvent(current, event),
      );
      if (current !== generation) {
        // Arrêté ou remplacé pendant l'ouverture : le flux n'a plus d'usage.
        void logs.stopLogs(id).catch(() => undefined);
        return;
      }
      streamId = id;
      status.value = "live";
      startingSince.value = null;
    } catch (raw) {
      if (current !== generation) return;
      finish("error");
      error.value = errorMessage(raw as AppError);
    }
  }

  /** Arrête le flux en cours (sans effet s'il n'y en a pas). */
  async function stop() {
    const id = streamId;
    const wasActive = status.value === "live" || status.value === "starting";
    if (wasActive) finish("stopped");
    else generation++;
    if (id !== null) await logs.stopLogs(id).catch(() => undefined);
  }

  function clear() {
    lines.value = [];
    received.value = 0;
    stderrCount.value = 0;
    errorCount.value = 0;
  }

  /** Rouvre le flux sur la même source (bouton « Rejouer »). */
  async function replay() {
    await stop();
    clear();
    await start();
  }

  watch(
    source,
    async (next, previous) => {
      if (previous !== undefined) await stop();
      clear();
      status.value = "idle";
      if (next !== null) await start();
    },
    { immediate: true, deep: true },
  );

  onBeforeUnmount(() => {
    unmounted = true;
    void stop();
  });

  const active = computed(
    () => status.value === "live" || status.value === "starting",
  );

  return {
    lines,
    status,
    active,
    exitCode,
    error,
    startingSince,
    endedAt,
    stderrCount,
    errorCount,
    received,
    stop,
    clear,
    replay,
  };
}

export type LogStream = ReturnType<typeof useLogStream>;
