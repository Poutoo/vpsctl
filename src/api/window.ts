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

// Boutons de la barre de titre personnalisée (permissions dans
// src-tauri/capabilities/default.json). Le déplacement et le double-clic
// passent par `data-tauri-drag-region`, géré par Tauri.

export function minimizeWindow(): Promise<void> {
  return getCurrentWindow().minimize();
}

export function toggleMaximizeWindow(): Promise<void> {
  return getCurrentWindow().toggleMaximize();
}

export function closeWindow(): Promise<void> {
  return getCurrentWindow().close();
}

/**
 * Garde la fermeture de la fenêtre (bouton Fermer, Alt+F4, gestionnaire de
 * fenêtres) : `allow` décide, et la fenêtre n'est gardée ouverte que s'il
 * refuse explicitement. S'il échoue (erreur, exception), elle se ferme
 * quand même : mieux vaut fermer que rester bloqué, et le script de
 * surveillance arrête les processus distants. Fermeture par
 * `destroy()` (permission `core:window:allow-destroy`). Renvoie la
 * fonction d'arrêt du suivi.
 */
export function guardWindowClose(
  allow: () => Promise<boolean>,
): Promise<() => void> {
  return getCurrentWindow().onCloseRequested(async (event) => {
    let allowed = true;
    try {
      allowed = await allow();
    } catch {
      allowed = true;
    }
    if (!allowed) event.preventDefault();
  });
}

/**
 * Suit l'état agrandi de la fenêtre (relu à chaque redimensionnement, ce qui
 * couvre aussi l'ancrage et le double-clic). Renvoie la fonction d'arrêt.
 */
export async function watchMaximized(
  onChange: (maximized: boolean) => void,
): Promise<() => void> {
  const win = getCurrentWindow();
  let check = 0;

  async function read() {
    const current = ++check;
    try {
      const maximized = await win.isMaximized();
      if (current === check) onChange(maximized);
    } catch {
      // Fenêtre indisponible (fermeture en cours) : état inchangé.
    }
  }

  void read();
  return win.onResized(() => void read());
}
