import { onBeforeUnmount, onMounted } from "vue";
import { useTelemetryStore } from "../stores/telemetry";

/**
 * Panneau ancré du tableau de bord : son flux s'arrête quand on quitte le
 * tableau de bord et repart au retour (la source choisie est gardée).
 * « Plein écran » le transfère avant le démontage : il continue alors dans
 * son onglet.
 */
export function useDockedStream() {
  const telemetry = useTelemetryStore();
  onMounted(() => telemetry.resumeDock());
  onBeforeUnmount(() => telemetry.suspendDock());
  return telemetry;
}
