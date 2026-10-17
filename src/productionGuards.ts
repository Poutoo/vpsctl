/**
 * Comportements du moteur web désactivés dans le build de production :
 *
 * - rechargement de la page (F5, Ctrl+R, Ctrl+Maj+R) : il interromprait sans
 *   confirmation les scripts en cours et fermerait tous les flux ;
 * - menu contextuel par défaut (Recharger, Précédent…), sauf dans un champ
 *   de saisie, où il sert à couper, copier et coller.
 *
 * Les menus de l'application (Reka UI) s'ouvrent sur l'élément visé, avant
 * que l'événement ne remonte jusqu'à `window` : ils ne sont pas touchés.
 */

function isReloadShortcut(event: KeyboardEvent): boolean {
  if (event.key === "F5") return true;
  return (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "r";
}

function isEditable(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    target.closest("input, textarea, [contenteditable='true']") !== null
  );
}

export function installProductionGuards() {
  // Phase de capture : aucun composant ne peut laisser passer le raccourci.
  window.addEventListener(
    "keydown",
    (event) => {
      if (isReloadShortcut(event)) event.preventDefault();
    },
    { capture: true },
  );
  window.addEventListener("contextmenu", (event) => {
    if (!isEditable(event.target)) event.preventDefault();
  });
}
