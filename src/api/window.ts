import { getCurrentWindow } from "@tauri-apps/api/window";

// Fenêtre de l'application (API de fenêtre de Tauri).

/**
 * Suit la visibilité réelle de la fenêtre : document visible **et** fenêtre
 * non réduite. Sous Windows, WebView2 ne passe pas `document.visibilityState`
 * à `hidden` quand la fenêtre est réduite : l'état « réduite » est donc relu
 * auprès de Tauri à chaque redimensionnement et changement de focus (une
 * réduction et une restauration déclenchent l'un ou l'autre).
 *
 * `onChange` n'est appelé qu'à un changement. Renvoie la fonction d'arrêt du
 * suivi et l'état initial.
 */
export async function watchWindowVisibility(
  onChange: (visible: boolean) => void,
): Promise<{ visible: boolean; stop: () => void }> {
  const win = getCurrentWindow();
  let documentVisible = document.visibilityState === "visible";
  let minimized = await readMinimized();
  let visible = documentVisible && !minimized;
  /** Ignore une lecture de `isMinimized` dépassée par une plus récente. */
  let check = 0;

  async function readMinimized(): Promise<boolean> {
    try {
      return await win.isMinimized();
    } catch {
      return false;
    }
  }

  function update() {
    const next = documentVisible && !minimized;
    if (next !== visible) {
      visible = next;
      onChange(visible);
    }
  }

  async function recheckMinimized() {
    const current = ++check;
    const value = await readMinimized();
    if (current !== check) return;
    minimized = value;
    update();
  }

  function onDocumentVisibility() {
    documentVisible = document.visibilityState === "visible";
    update();
  }

  document.addEventListener("visibilitychange", onDocumentVisibility);
  const unlisten = await Promise.all([
    win.onResized(() => void recheckMinimized()),
    win.onFocusChanged(() => void recheckMinimized()),
  ]);

  return {
    visible,
    stop() {
      document.removeEventListener("visibilitychange", onDocumentVisibility);
      for (const off of unlisten) off();
    },
  };
}
