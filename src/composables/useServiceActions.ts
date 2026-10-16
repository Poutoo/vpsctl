import type { Container } from "../bindings/Container";
import type { Service } from "../bindings/Service";
import type { ServiceOperation } from "../components/features/availability";
import {
  containerTarget,
  serviceTarget,
  useServicesStore,
  type Operation,
} from "../stores/services";
import { useConfirm, type ConfirmOptions } from "./useConfirm";

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

/** Fenêtre GO / NO GO d'une action sur un service ; `null` : action directe. */
function serviceConfirmation(
  service: Service,
  operation: ServiceOperation,
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

  async function onContainer(container: Container, operation: Operation) {
    const options = containerConfirmation(container, operation);
    if (options && !(await confirm(options))) return;
    await store.perform(containerTarget(container), operation);
  }

  async function onService(service: Service, operation: ServiceOperation) {
    const options = serviceConfirmation(service, operation);
    if (options && !(await confirm(options))) return;
    await store.perform(serviceTarget(service), operation);
  }

  return { onContainer, onService };
}
