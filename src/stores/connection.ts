import { computed, ref } from "vue";
import { defineStore } from "pinia";
import * as api from "../api/connection";
import type { AppError } from "../bindings/AppError";
import type { ConnectionInfo } from "../bindings/ConnectionInfo";
import type { ConnectStep } from "../bindings/ConnectStep";
import type { ServerInfo } from "../bindings/ServerInfo";

/** Intervalle de rafraîchissement de l'état (aligné sur la mesure de latence). */
const STATUS_INTERVAL_MS = 30_000;

export type ConnectPhase =
  | "idle"
  | "connecting"
  | "hostKeyUnknown"
  | "hostKeyMismatch"
  | "passphrase"
  | "failed"
  | "connected";

/** Étape reçue, avec son instant relatif au début de la tentative. */
export interface TimedStep {
  step: ConnectStep;
  atMs: number;
}

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

  const isConnected = computed(() => info.value !== null);

  let attempt = 0;
  let initialized: Promise<void> | null = null;
  let statusTimer: ReturnType<typeof setInterval> | null = null;

  /** Lit une seule fois l'état du backend (utile après un rechargement). */
  function initialize(): Promise<void> {
    initialized ??= refreshStatus().catch(() => undefined);
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
    } else if (info.value || phase.value === "connected") {
      clearConnected();
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

  function clearConnected() {
    stopStatusPolling();
    info.value = null;
    serverInfo.value = null;
    phase.value = "idle";
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
      info.value = connected;
      phase.value = "connected";
      passphraseRejected.value = false;
      startStatusPolling();
      void loadServerInfo();
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
        break;
      case "HostKeyMismatch":
        expectedFingerprint.value = failure.expected;
        presentedFingerprint.value = failure.received;
        phase.value = "hostKeyMismatch";
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
        phase.value = "idle";
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

  /** Interrompt la tentative en cours. */
  async function cancel() {
    attempt++;
    phase.value = "idle";
    await api.disconnect();
  }

  async function disconnect() {
    attempt++;
    await api.disconnect();
    clearConnected();
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
    isConnected,
    initialize,
    refreshStatus,
    connect,
    submitPassphrase,
    trustAndReconnect,
    cancel,
    disconnect,
    reset,
  };
});
