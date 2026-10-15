<script setup lang="ts">
/**
 * Indicateur de liaison de la barre de titre : AOS (établie), ACQUISITION
 * (en cours) ou LOS (aucune, perdue, échouée). Forme et libellé : lisible
 * sans la couleur. Exclu de la zone de déplacement de la fenêtre.
 */
import { computed } from "vue";
import { useConnectionStore } from "../../stores/connection";
import { useProfilesStore } from "../../stores/profiles";
import AppIcon, { type AppIconName } from "../ui/AppIcon.vue";
import AppTooltip from "../ui/AppTooltip.vue";

const connection = useConnectionStore();
const profiles = useProfilesStore();

interface Signal {
  tone: "aos" | "acquisition" | "idle" | "lost";
  icon: AppIconName;
  code: string;
  detail: string;
  latency: string | null;
  tooltip: string;
}

const signal = computed<Signal>(() => {
  const name = profiles.byId(connection.profileId)?.name ?? "profil";
  if (connection.isConnected) {
    const latency = connection.info?.latencyMs;
    return {
      tone: "aos",
      icon: "signal",
      code: "AOS",
      detail: name,
      latency: latency != null ? `${latency} ms` : null,
      tooltip: "Liaison établie · latence mesurée toutes les 30 s",
    };
  }
  switch (connection.phase) {
    case "connecting":
      return {
        tone: "acquisition",
        icon: "dish",
        code: "ACQUISITION",
        detail: name,
        latency: null,
        tooltip: "Connexion en cours",
      };
    case "hostKeyUnknown":
    case "passphrase":
      return {
        tone: "acquisition",
        icon: "dish",
        code: "ACQUISITION",
        detail: "vérification requise",
        latency: null,
        tooltip: "La connexion attend une vérification de votre part",
      };
    case "hostKeyMismatch":
      return {
        tone: "lost",
        icon: "signal-lost",
        code: "LOS",
        detail: "liaison coupée",
        latency: null,
        tooltip: "L'empreinte du serveur a changé : liaison bloquée",
      };
    case "failed":
      return {
        tone: "lost",
        icon: "signal-lost",
        code: "LOS",
        detail: "échec de liaison",
        latency: null,
        tooltip: "La dernière tentative de connexion a échoué",
      };
    default:
      return {
        tone: "idle",
        icon: "signal-lost",
        code: "LOS",
        detail: "aucune liaison",
        latency: null,
        tooltip: "Aucune liaison active",
      };
  }
});

const label = computed(() =>
  [signal.value.code, signal.value.detail, signal.value.latency]
    .filter(Boolean)
    .join(" · "),
);
</script>

<template>
  <AppTooltip :text="signal.tooltip">
    <span
      class="indicator"
      :class="signal.tone"
      role="status"
      tabindex="0"
      :aria-label="label"
      data-tauri-drag-region="false"
    >
      <AppIcon class="icon" :name="signal.icon" />
      <span class="caps code">{{ signal.code }}</span>
      <span class="separator" aria-hidden="true">·</span>
      <span class="detail">{{ signal.detail }}</span>
      <template v-if="signal.latency">
        <span class="separator" aria-hidden="true">·</span>
        <span class="latency">{{ signal.latency }}</span>
      </template>
    </span>
  </AppTooltip>
</template>

<style scoped>
.indicator {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  max-width: 100%;
  padding: var(--sp-1) var(--sp-3);
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-1);
  color: var(--color-text);
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
  white-space: nowrap;
  cursor: default;
}

.code {
  font-weight: 600;
}

.separator,
.latency {
  color: var(--color-text-muted);
}

.detail {
  overflow: hidden;
  text-overflow: ellipsis;
}

.aos {
  border-color: var(--color-success);
}

.aos .icon,
.aos .code {
  color: var(--color-success);
}

.acquisition {
  border-color: var(--color-warning);
}

.acquisition .icon,
.acquisition .code {
  color: var(--color-warning);
}

.idle .icon,
.idle .code {
  color: var(--color-text-muted);
}

/* Liaison perdue : forme en --color-danger, texte en --color-danger-text. */
.lost {
  background: var(--color-danger-tint);
  border-color: var(--color-danger);
}

.lost .icon {
  color: var(--color-danger);
}

.lost .code {
  color: var(--color-danger-text);
}
</style>
