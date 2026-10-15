import { Channel } from "@tauri-apps/api/core";
import type { ConnectionInfo } from "../bindings/ConnectionInfo";
import type { ConnectStep } from "../bindings/ConnectStep";
import type { Profile } from "../bindings/Profile";
import type { ServerInfo } from "../bindings/ServerInfo";
import { call } from "./invoke";

// Connexion et profils (docs/04-contrat-commandes.md).

export function listProfiles(): Promise<Profile[]> {
  return call("list_profiles");
}

export function saveProfile(profile: Profile): Promise<Profile> {
  return call("save_profile", { profile });
}

export function deleteProfile(id: string): Promise<void> {
  return call("delete_profile", { id });
}

export function setKeyPassphrase(
  profileId: string,
  passphrase: string,
): Promise<void> {
  return call("set_key_passphrase", { profileId, passphrase });
}

export function trustHostKey(
  profileId: string,
  fingerprint: string,
): Promise<void> {
  return call("trust_host_key", { profileId, fingerprint });
}

/**
 * Établit la liaison. `passphrase` est à usage unique (jamais mémorisée) ;
 * `null` utilise celle du trousseau, si elle existe.
 */
export function connect(
  profileId: string,
  passphrase: string | null,
  onStep: (step: ConnectStep) => void,
): Promise<ConnectionInfo> {
  const channel = new Channel<ConnectStep>();
  channel.onmessage = onStep;
  return call("connect", { profileId, passphrase, onStep: channel });
}

/** Ferme la liaison, ou interrompt une connexion en cours. */
export function disconnect(): Promise<void> {
  return call("disconnect");
}

export function connectionStatus(): Promise<ConnectionInfo | null> {
  return call("connection_status");
}

export function serverInfo(): Promise<ServerInfo> {
  return call("server_info");
}
