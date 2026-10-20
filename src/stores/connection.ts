import { computed, ref } from "vue";
import { defineStore } from "pinia";
import * as api from "../api/connection";
import type { AppError } from "../bindings/AppError";
import type { ConnectionInfo } from "../bindings/ConnectionInfo";
import type { ConnectStep } from "../bindings/ConnectStep";
import type { LinkLost } from "../bindings/LinkLost";
import type { LossCause } from "../bindings/LossCause";
import type { ServerInfo } from "../bindings/ServerInfo";

/** Intervalle de mesure de la latence (aligné sur celle du backend). */
const STATUS_INTERVAL_MS = 30_000;
/** Durée d'affichage du bandeau « Liaison rétablie ». */
const RESTORED_MS = 5_000;

export type ConnectPhase =
  | "idle"
  | "connecting"
  | "hostKeyUnknown"
  | "hostKeyMismatch"
  | "passphrase"
  | "failed"
  | "connected"
  /** Liaison perdue, aucune tentative en cours. */
  | "lost";

/** Étape reçue, avec son instant relatif au début de la tentative. */
export interface TimedStep {
  step: ConnectStep;
  atMs: number;
}

/** Liaison perdue : l'application reste en LOS jusqu'au rétablissement. */
export interface LinkLoss {
  profileId: string;
  cause: LossCause;
  detail: string | null;
  /** Dernière réception de données du serveur (horodatage en ms). */
  lastReceivedAt: number | null;
  /** Début du LOS (horodatage local en ms). */
  since: number;
  /** Clics sur « Rétablir la liaison ». */
  attempts: number;
  /** Constatée par le sondage, avant l'événement du backend. */
  provisional: boolean;
}

export interface LinkRestored {
  outageMs: number;
  at: number;
}

/**
 * Moments de la liaison auxquels les autres stores réagissent :
 * - `lost` : LOS, données figées, flux et transferts interrompus ;
 * - `restored` : liaison rétablie après un LOS (rien ne repart seul) ;
 * - `ended` : fin de session voulue (déconnexion, abandon du LOS).
 */
export type LinkMoment = "lost" | "restored" | "ended";

export const useConnectionStore = defineStore("connection", () => {
  const info = ref<ConnectionInfo | null>(null);
  const serverInfo = ref<ServerInfo | null>(null);
  const phase = ref<ConnectPhase>("idle");
  const profileId = ref<string | null>(null);
  const steps = ref<TimedStep[]>([]);
  const error = ref<AppError | null>(null);
  /** Empreinte présentée par le serveur (clé inconnue ou différente). */
  const presentedFingerprint = ref<string | null>(null);
  /** Empreinte mémorisée dans le profil (clé différente). */
  const expectedFingerprint = ref<string | null>(null);
  /** La dernière passphrase saisie était incorrecte. */
  const passphraseRejected = ref(false);
  const loss = ref<LinkLoss | null>(null);
  const restored = ref<LinkRestored | null>(null);

  const isConnected = computed(() => info.value !== null);
  const lost = computed(() => loss.value !== null);
  /** Session ouverte : liaison établie, ou perdue en attente de rétablissement. */
  const hasSession = computed(() => isConnected.value || lost.value);

  let attempt = 0;
  let initialized: Promise<void> | null = null;
  let statusTimer: ReturnType<typeof setInterval> | null = null;
  let restoredTimer: ReturnType<typeof setTimeout> | null = null;
  /** Déconnexion volontaire en cours : une perte signalée est ignorée. */
  let leaving = false;
  const listeners: Record<LinkMoment, Set<() => void>> = {
    lost: new Set(),
    restored: new Set(),
    ended: new Set(),
  };

  function on(moment: LinkMoment, listener: () => void): () => void {
    listeners[moment].add(listener);
    return () => listeners[moment].delete(listener);
  }

  function announce(moment: LinkMoment) {
    for (const listener of listeners[moment]) listener();
  }

  /** Lit l'état du backend et écoute `link-lost`, une seule fois. */
  function initialize(): Promise<void> {
    initialized ??= Promise.all([
      api.onLinkLost(linkLost),
      refreshStatus(),
    ]).then(
      () => undefined,
      () => undefined,
    );
    return initialized;
  }

  async function refreshStatus() {
    const status = await api.connectionStatus();
    if (status) {
      info.value = status;
      profileId.value = status.profileId;
      phase.value = "connected";
      startStatusPolling();
      if (!serverInfo.value) void loadServerInfo();
    } else if (info.value) {
      linkLost(null);
    }
  }

  async function loadServerInfo() {
    try {
      serverInfo.value = await api.serverInfo();
    } catch {
      serverInfo.value = null;
    }
  }

  function startStatusPolling() {
    if (statusTimer) return;
    statusTimer = setInterval(() => void refreshStatus(), STATUS_INTERVAL_MS);
  }

  function stopStatusPolling() {
    if (statusTimer) clearInterval(statusTimer);
    statusTimer = null;
  }

  function clearRestored() {
    if (restoredTimer) clearTimeout(restoredTimer);
    restoredTimer = null;
    restored.value = null;
  }

  /**
   * Point d'entrée unique de la perte de liaison : événement `link-lost` du
   * backend, ou, à défaut, sondage qui trouve la liaison absente (`null`,
   * cause précisée si l'événement arrive ensuite).
   */
  function linkLost(event: LinkLost | null) {
    if (leaving) return;
    const current = loss.value;
    if (current) {
      if (current.provisional && event) {
        Object.assign(current, {
          cause: event.cause,
          detail: event.detail,
          lastReceivedAt: event.lastReceivedAt,
          provisional: false,
        });
      }
      return;
    }
    const linked = info.value;
    if (!linked || (event && event.profileId !== linked.profileId)) return;

    attempt++;
    stopStatusPolling();
    clearRestored();
    loss.value = {
      profileId: linked.profileId,
      cause: event?.cause ?? "unknown",
      detail: event?.detail ?? null,
      lastReceivedAt: event?.lastReceivedAt ?? null,
      since: Date.now(),
      attempts: 0,
      provisional: event === null,
    };
    info.value = null;
    phase.value = "lost";
    error.value = null;
    announce("lost");
  }

  /** « Rétablir la liaison » : rejoue la séquence de connexion, une tentative par clic. */
  async function reacquire() {
    const current = loss.value;
    if (!current || phase.value === "connecting") return;
    current.attempts++;
    await connect(current.profileId);
  }

  function restore(outageMs: number) {
    loss.value = null;
    clearRestored();
    restored.value = { outageMs, at: Date.now() };
    restoredTimer = setTimeout(clearRestored, RESTORED_MS);
    announce("restored");
  }

  /** Quitte la session : déconnexion voulue ou abandon du LOS. */
  function endSession(hadSession = hasSession.value) {
    loss.value = null;
    clearRestored();
    if (hadSession) announce("ended");
  }

  /** Lance une nouvelle tentative de connexion. */
  async function connect(id: string, passphrase: string | null = null) {
    const current = ++attempt;
    const startedAt = performance.now();
    profileId.value = id;
    phase.value = "connecting";
    steps.value = [];
    error.value = null;
    presentedFingerprint.value = null;
    expectedFingerprint.value = null;

    try {
      const connected = await api.connect(id, passphrase, (step) => {
        if (current !== attempt) return;
        steps.value.push({ step, atMs: performance.now() - startedAt });
      });
      if (current !== attempt) return;
      const outage = loss.value ? Date.now() - loss.value.since : null;
      info.value = connected;
      phase.value = "connected";
      passphraseRejected.value = false;
      startStatusPolling();
      void loadServerInfo();
      if (outage !== null) restore(outage);
    } catch (raw) {
      if (current !== attempt) return;
      handleFailure(raw as AppError);
    }
  }

  function handleFailure(failure: AppError) {
    error.value = failure;
    switch (failure.kind) {
      case "UnknownHostKey":
        presentedFingerprint.value = failure.fingerprint;
        phase.value = "hostKeyUnknown";
        endSession();
        break;
      case "HostKeyMismatch":
        // Alerte de sécurité : on quitte le LOS pour l'écran dédié.
        expectedFingerprint.value = failure.expected;
        presentedFingerprint.value = failure.received;
        phase.value = "hostKeyMismatch";
        endSession();
        break;
      case "PassphraseRequired":
        passphraseRejected.value = false;
        phase.value = "passphrase";
        break;
      case "PassphraseIncorrect":
        passphraseRejected.value = true;
        phase.value = "passphrase";
        break;
      case "Cancelled":
        phase.value = lost.value ? "lost" : "idle";
        break;
      default:
        phase.value = "failed";
    }
  }

  /**
   * Passphrase saisie : mémorisée (après vérification locale par le backend)
   * puis connexion, ou utilisée une seule fois.
   */
  async function submitPassphrase(passphrase: string, remember: boolean) {
    const id = profileId.value;
    if (!id) return;
    passphraseRejected.value = false;
    if (remember) {
      try {
        await api.setKeyPassphrase(id, passphrase);
      } catch (raw) {
        const failure = raw as AppError;
        if (failure.kind === "PassphraseIncorrect") {
          passphraseRejected.value = true;
          return;
        }
        handleFailure(failure);
        return;
      }
      await connect(id, null);
    } else {
      await connect(id, passphrase);
    }
  }

  /** Accepte l'empreinte présentée puis lance une nouvelle connexion. */
  async function trustAndReconnect() {
    const id = profileId.value;
    const fingerprint = presentedFingerprint.value;
    if (!id || !fingerprint) return;
    try {
      await api.trustHostKey(id, fingerprint);
    } catch (raw) {
      handleFailure(raw as AppError);
      return;
    }
    await connect(id, null);
  }

  /** Interrompt la tentative en cours ; en LOS, on y revient. */
  async function cancel() {
    attempt++;
    phase.value = lost.value ? "lost" : "idle";
    passphraseRejected.value = false;
    await api.disconnect();
  }

  /** Déconnexion volontaire : jamais de LOS, la session se termine. */
  async function disconnect() {
    attempt++;
    leaving = true;
    const hadSession = hasSession.value;
    try {
      await api.disconnect();
    } finally {
      leaving = false;
      clearConnected();
      endSession(hadSession);
    }
  }

  function clearConnected() {
    stopStatusPolling();
    info.value = null;
    serverInfo.value = null;
    phase.value = "idle";
  }

  /** Revient à la liste des profils (refus d'empreinte, abandon). */
  function reset() {
    attempt++;
    phase.value = "idle";
    error.value = null;
    presentedFingerprint.value = null;
    expectedFingerprint.value = null;
    passphraseRejected.value = false;
  }

  return {
    info,
    serverInfo,
    phase,
    profileId,
    steps,
    error,
    presentedFingerprint,
    expectedFingerprint,
    passphraseRejected,
    loss,
    restored,
    isConnected,
    lost,
    hasSession,
    on,
    initialize,
    refreshStatus,
    linkLost,
    reacquire,
    connect,
    submitPassphrase,
    trustAndReconnect,
    cancel,
    disconnect,
    dismissRestored: clearRestored,
    reset,
  };
});
