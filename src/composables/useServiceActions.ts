import { errorMessage } from "../api/errors";
import * as systemd from "../api/systemd";
import type { AppError } from "../bindings/AppError";
import type { Container } from "../bindings/Container";
import type { Service } from "../bindings/Service";
import type { ServiceDeleteInfo } from "../bindings/ServiceDeleteInfo";
import type { ServiceOperation } from "../components/features/availability";
import {
  containerTarget,
  serviceTarget,
  useServicesStore,
  type Operation,
} from "../stores/services";
import { useConfirm, type ConfirmOptions } from "./useConfirm";
import { useNotify } from "./useNotify";

/** Fenêtre GO / NO GO d'une action sur un conteneur ; `null` : action directe. */
function containerConfirmation(
  container: Container,
  operation: Operation,
): ConfirmOptions | null {
  switch (operation) {
    case "start":
      return null;
    case "stop":
      return {
        title: "Arrêter le conteneur ?",
        subject: container.name,
        consequences: [
          "Le conteneur reçoit SIGTERM, puis SIGKILL s'il ne s'arrête pas dans son délai d'arrêt (10 s par défaut).",
          "Les connexions en cours sont coupées.",
          "Il reste arrêté jusqu'à un démarrage manuel ou un redémarrage de Docker.",
        ],
        confirmLabel: "Arrêter",
      };
    case "restart":
      return {
        title: "Redémarrer le conteneur ?",
        subject: container.name,
        consequences: [
          "Le conteneur est arrêté (SIGTERM, puis SIGKILL après son délai d'arrêt), puis relancé.",
          "Le service qu'il assure est interrompu pendant le redémarrage.",
          "Les connexions en cours sont coupées.",
        ],
        confirmLabel: "Redémarrer",
      };
    case "remove":
      return {
        title: "Supprimer le conteneur ?",
        subject: container.name,
        checks: [{ label: "Conteneur arrêté", detail: container.status }],
        consequences: [
          "Le conteneur et ses fichiers propres, hors volumes, sont supprimés définitivement.",
          "Les volumes sont conservés.",
          `L'image est conservée : ${container.image}.`,
        ],
        confirmLabel: "Supprimer",
        danger: true,
      };
  }
}

/** GO / NO GO d'une suppression de service, d'après `service_delete_info`. */
function deleteConfirmation(
  service: Service,
  info: ServiceDeleteInfo,
): ConfirmOptions {
  const consequences: ConfirmOptions["consequences"] = [];
  if (info.active) {
    consequences.push(
      "Le service est arrêté (SIGTERM, puis SIGKILL après son délai d'arrêt) ; la suppression attend la fin de l'arrêt.",
    );
  }
  consequences.push(
    "Il est désactivé : il ne démarrera plus avec la session de l'utilisateur.",
    { label: "Fichier d'unité supprimé", code: info.file },
  );
  if (info.dropIn !== null) {
    const entries =
      info.dropInEntries === 0
        ? ""
        : info.dropInEntries === 1
          ? ", avec son élément"
          : `, avec ses ${info.dropInEntries} éléments`;
    consequences.push({
      label: `Dossier de surcharge supprimé${entries}`,
      code: info.dropIn,
    });
  }
  consequences.push("systemd est rechargé (daemon-reload).");
  return {
    title: "Supprimer le service ?",
    subject: info.unit,
    checks: info.active
      ? []
      : [
          {
            label: "Service arrêté",
            detail: `${service.active}/${service.sub}`,
          },
        ],
    consequences,
    confirmLabel: "Supprimer",
    danger: true,
  };
}

/** Fenêtre GO / NO GO d'une action sur un service ; `null` : action directe. */
function serviceConfirmation(
  service: Service,
  operation: Exclude<ServiceOperation, "remove">,
): ConfirmOptions | null {
  switch (operation) {
    case "start":
      return null;
    case "stop":
      return {
        title: "Arrêter le service ?",
        subject: service.unit,
        consequences: [
          "Le service reçoit SIGTERM, puis SIGKILL s'il ne s'arrête pas dans son délai d'arrêt (TimeoutStopSec).",
          "Il reste arrêté jusqu'à un démarrage manuel, ou jusqu'au prochain démarrage du serveur s'il est activé.",
        ],
        confirmLabel: "Arrêter",
      };
    case "restart":
      return {
        title: "Redémarrer le service ?",
        subject: service.unit,
        consequences: [
          "Le service est arrêté puis relancé.",
          "Il est indisponible pendant le redémarrage.",
        ],
        confirmLabel: "Redémarrer",
      };
  }
}

/**
 * Actions du tableau de bord : confirmation GO / NO GO pour arrêter,
 * redémarrer et supprimer (démarrer est direct), puis exécution et
 * vérification par le store `services`.
 */
export function useServiceActions() {
  const store = useServicesStore();
  const { confirm } = useConfirm();
  const { notify } = useNotify();

  async function onContainer(container: Container, operation: Operation) {
    const options = containerConfirmation(container, operation);
    if (options && !(await confirm(options))) return;
    await store.perform(containerTarget(container), operation);
  }

  async function onService(service: Service, operation: ServiceOperation) {
    if (operation === "remove") return removeService(service);
    const options = serviceConfirmation(service, operation);
    if (options && !(await confirm(options))) return;
    await store.perform(serviceTarget(service), operation);
  }

  /**
   * Suppression : le fichier et le dossier de surcharge sont relus par le
   * backend avant le GO / NO GO, qui les liste.
   */
  async function removeService(service: Service) {
    let info: ServiceDeleteInfo;
    try {
      info = await systemd.serviceDeleteInfo(service.unit);
    } catch (error) {
      notify({
        tone: "error",
        title: "Suppression impossible",
        detail: service.unit,
        message: errorMessage(error as AppError),
      });
      return;
    }
    if (!(await confirm(deleteConfirmation(service, info)))) return;
    await store.perform(serviceTarget(service), "remove");
  }

  return { onContainer, onService };
}
