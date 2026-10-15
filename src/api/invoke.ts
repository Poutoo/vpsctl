import { invoke } from "@tauri-apps/api/core";
import { toAppError } from "./errors";

/** Appelle une commande Tauri ; toute erreur est rejetée sous forme d'`AppError`. */
export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toAppError(error);
  }
}
