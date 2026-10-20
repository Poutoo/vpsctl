/**
 * Ctrl+F : place le curseur dans le filtre de l'écran affiché et
 * sélectionne son texte. Le raccourci est toujours intercepté, même sur un
 * écran sans filtre : la barre de recherche du moteur web (WebView2) ne
 * s'ouvre jamais. Une fenêtre modale ouverte garde la main.
 */

export interface FindTarget {
  focus(): void;
  /** Faux quand le filtre ne doit pas prendre le focus (saisie en cours). */
  enabled(): boolean;
}

let current: FindTarget | null = null;

/** Déclare le filtre de l'écran affiché ; renvoie la fonction de retrait. */
export function registerFindTarget(target: FindTarget): () => void {
  current = target;
  return () => {
    if (current === target) current = null;
  };
}

export function isFindShortcut(event: KeyboardEvent): boolean {
  return (
    (event.ctrlKey || event.metaKey) &&
    !event.altKey &&
    !event.shiftKey &&
    event.key.toLowerCase() === "f"
  );
}

function modalOpen(root: Document): boolean {
  return (
    root.querySelector(
      '[role="dialog"][data-state="open"], [role="alertdialog"][data-state="open"]',
    ) !== null
  );
}

export function onFindKeydown(event: KeyboardEvent) {
  if (!isFindShortcut(event)) return;
  event.preventDefault();
  if (current && current.enabled() && !modalOpen(document)) current.focus();
}

/** Phase de capture : aucun composant ne peut laisser passer le raccourci. */
export function installFindShortcut(target: Window = window): () => void {
  target.addEventListener("keydown", onFindKeydown, { capture: true });
  return () =>
    target.removeEventListener("keydown", onFindKeydown, { capture: true });
}
