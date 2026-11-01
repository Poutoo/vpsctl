import { useRouter } from "vue-router";
import { errorMessage } from "../api/errors";
import * as composeApi from "../api/compose";
import type { AppError } from "../bindings/AppError";
import type { ComposeAction } from "../bindings/ComposeAction";
import type { ComposeProject } from "../bindings/ComposeProject";
import { downConfirmation } from "../composeActions";
import { useComposeStore } from "../stores/compose";
import { useConnectionStore } from "../stores/connection";
import { useTelemetryStore } from "../stores/telemetry";
import { useConfirm } from "./useConfirm";
import { useNotify } from "./useNotify";

/**
 * Actions de projet compose : GO / NO GO pour « Arrêter et supprimer »
 * (contenu lu sur le serveur juste avant), puis ouverture de l'écran
 * Télémétrie sur l'onglet de la commande. Une commande déjà en cours sur le
 * projet : son onglet s'ouvre, sans nouvelle exécution.
 */
export function useComposeActions() {
  const telemetry = useTelemetryStore();
  const compose = useComposeStore();
  const connection = useConnectionStore();
  const router = useRouter();
  const { confirm } = useConfirm();
  const { notify } = useNotify();

  function showTab(id: string) {
    void router.push({ name: "telemetry", params: { tab: id } });
  }

  async function confirmDown(project: ComposeProject): Promise<boolean> {
    let info;
    try {
      info = await composeApi.downInfo(project.name);
    } catch (raw) {
      if ((raw as AppError).kind === "LinkLost") return false;
      notify({
        tone: "error",
        title: "Arrêt du projet impossible",
        detail: project.name,
        message: errorMessage(raw as AppError),
      });
      return false;
    }
    return confirm(downConfirmation(info));
  }

  async function run(project: ComposeProject, action: ComposeAction) {
    const running = telemetry.composeRunning(project.name);
    if (running) {
      showTab(running.id);
      return;
    }
    if (connection.lost) return;
    if (action === "down" && !(await confirmDown(project))) return;
    const tab = await compose.run(project, action);
    if (tab) showTab(tab.id);
  }

  return { run };
}
