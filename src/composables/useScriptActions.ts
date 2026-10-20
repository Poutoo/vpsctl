import { useRouter } from "vue-router";
import type { Script } from "../bindings/Script";
import { useConnectionStore } from "../stores/connection";
import { useTelemetryStore } from "../stores/telemetry";
import { useConfirm } from "./useConfirm";

/**
 * Exécution d'un script : GO / NO GO pour un script marqué
 * `requireConfirmation` (la commande est listée dans les conséquences),
 * puis ouverture de l'écran Télémétrie sur son onglet. Déjà en cours :
 * l'onglet s'ouvre, sans nouvelle exécution.
 */
export function useScriptActions() {
  const telemetry = useTelemetryStore();
  const connection = useConnectionStore();
  const router = useRouter();
  const { confirm } = useConfirm();

  function showTab(id: string) {
    void router.push({ name: "telemetry", params: { tab: id } });
  }

  async function run(script: Script) {
    const running = telemetry.scriptTab(script.id);
    if (running?.session.active) {
      showTab(running.id);
      return;
    }
    // Jamais de relance pendant un LOS : on attend le rétablissement.
    if (connection.lost) return;
    if (script.requireConfirmation) {
      const confirmed = await confirm({
        title: "Exécuter la procédure ?",
        subject: script.name,
        consequences: [
          "Confirmation obligatoire avant chaque exécution.",
          { label: "Commande", code: script.command },
          "Exécutée sur le serveur dans le shell de connexion, sans saisie possible.",
          "La sortie s'affiche dans la télémétrie.",
        ],
        confirmLabel: "Exécuter",
      });
      if (!confirmed) return;
    }
    const tab = await telemetry.runScript(script);
    if (tab) showTab(tab.id);
  }

  return { run };
}
