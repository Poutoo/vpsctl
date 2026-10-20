<script setup lang="ts">
/**
 * Bandeau de liaison (planche III), au-dessus du contenu et hors défilement :
 * - LOS : non refermable tant que la liaison manque, durée qui défile, seul
 *   bouton « Rétablir la liaison » (jamais de reconnexion automatique) ;
 * - ACQ : tentative en cours (un clic = une tentative), « Annuler » ; si la
 *   passphrase n'est pas mémorisée, son panneau s'ouvre par-dessus ;
 * - AOS : « Liaison rétablie » pendant 5 s, ou jusqu'à « Fermer ».
 */
import {
  DialogContent,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "reka-ui";
import { computed } from "vue";
import { errorMessage } from "../../api/errors";
import { useElapsed } from "../../composables/useElapsed";
import { causeLabel, outageLabel, timeLabel } from "../../linkText";
import { useConnectionStore } from "../../stores/connection";
import { useProfilesStore } from "../../stores/profiles";
import { useServicesStore } from "../../stores/services";
import { useTelemetryStore } from "../../stores/telemetry";
import PassphrasePrompt from "../features/PassphrasePrompt.vue";
import AppButton from "../ui/AppButton.vue";
import AppIcon from "../ui/AppIcon.vue";

const connection = useConnectionStore();
const profiles = useProfilesStore();
const services = useServicesStore();
const telemetry = useTelemetryStore();

const clock = { clock: () => Date.now(), tickMs: 1_000 };
const lostFor = useElapsed(
  computed(() => connection.loss?.since ?? null),
  clock,
);
const attemptStartedAt = computed(() =>
  connection.lost && connection.phase === "connecting" ? Date.now() : null,
);
const attemptFor = useElapsed(attemptStartedAt, clock);

const profile = computed(() =>
  profiles.byId(connection.loss?.profileId ?? connection.profileId),
);
const name = computed(() => profile.value?.name ?? "le serveur");
const endpoint = computed(() =>
  profile.value
    ? `${profile.value.user}@${profile.value.host}:${profile.value.port}`
    : "",
);

const mode = computed<"lost" | "acquiring" | "passphrase" | "restored" | null>(
  () => {
    if (connection.lost) {
      if (connection.phase === "connecting") return "acquiring";
      if (connection.phase === "passphrase") return "passphrase";
      return "lost";
    }
    return connection.restored ? "restored" : null;
  },
);

const lostDetail = computed(() => {
  const loss = connection.loss;
  if (!loss) return "";
  const parts = [`Depuis ${outageLabel(lostFor.value ?? 0)}`];
  if (loss.lastReceivedAt !== null) {
    parts.push(`dernier échange ${timeLabel(loss.lastReceivedAt)}`);
  }
  parts.push(causeLabel(loss.cause, loss.detail));
  return parts.join(" · ");
});

/** Échec de la dernière tentative, affiché sous le détail du LOS. */
const failure = computed(() =>
  connection.phase === "failed" && connection.error
    ? `Tentative ${connection.loss?.attempts ?? 1} échouée : ${errorMessage(connection.error)}`
    : null,
);

const acquiringDetail = computed(() =>
  [
    `Tentative ${connection.loss?.attempts ?? 1}`,
    endpoint.value,
    outageLabel(attemptFor.value ?? 0),
    `coupure de ${outageLabel(lostFor.value ?? 0)}`,
  ]
    .filter(Boolean)
    .join(" · "),
);

const restoredDetail = computed(() => {
  const restored = connection.restored;
  if (!restored) return "";
  const refreshed = services.refreshedAt;
  const parts = [
    `Coupure de ${outageLabel(restored.outageMs)}`,
    refreshed !== null && refreshed >= restored.at
      ? `données actualisées à ${timeLabel(refreshed)}`
      : "actualisation des données…",
  ];
  const streams = [telemetry.dock, ...telemetry.tabs];
  if (streams.some((s) => s?.kind === "logs" && s.session.status === "lost")) {
    parts.push("flux de journal à reprendre dans la console");
  }
  return parts.join(" · ");
});

const busy = computed(() => connection.phase === "connecting");

function submitPassphrase(passphrase: string, remember: boolean) {
  void connection.submitPassphrase(passphrase, remember);
}
</script>

<template>
  <div v-if="mode" class="banner" :class="mode" role="status">
    <template v-if="mode === 'lost'">
      <AppIcon class="icon" name="signal-lost" />
      <div class="text">
        <p class="title">
          <span class="caps code">LOS</span>
          Liaison perdue avec {{ name }}
        </p>
        <p class="detail">{{ lostDetail }}</p>
        <p v-if="failure" class="detail failure" role="alert">{{ failure }}</p>
      </div>
      <AppButton variant="primary" @click="connection.reacquire()">
        Rétablir la liaison
      </AppButton>
    </template>

    <template v-else-if="mode === 'acquiring' || mode === 'passphrase'">
      <AppIcon class="icon" name="dish" />
      <div class="text">
        <p class="title">
          <span class="caps code">ACQ</span>
          <template v-if="mode === 'acquiring'">
            Rétablissement de la liaison…
          </template>
          <template v-else>
            Rétablissement suspendu · passphrase requise
          </template>
        </p>
        <p class="detail">
          <template v-if="mode === 'acquiring'">{{ acquiringDetail }}</template>
          <template v-else>
            Tentative {{ connection.loss?.attempts ?? 1 }} · empreinte vérifiée
            · clé {{ profile?.keyPath }} chiffrée
          </template>
        </p>
      </div>
      <AppButton @click="connection.cancel()">Annuler</AppButton>
    </template>

    <template v-else>
      <AppIcon class="icon" name="success" />
      <div class="text">
        <p class="title">
          <span class="caps code">AOS</span>
          Liaison rétablie
        </p>
        <p class="detail">{{ restoredDetail }}</p>
      </div>
      <AppButton @click="connection.dismissRestored()">Fermer</AppButton>
    </template>
  </div>

  <DialogRoot
    :open="mode === 'passphrase'"
    @update:open="(open) => !open && connection.cancel()"
  >
    <DialogPortal>
      <DialogOverlay class="overlay" />
      <DialogContent class="dialog" :aria-describedby="undefined">
        <DialogTitle class="visually-hidden">
          Déverrouiller la clé privée
        </DialogTitle>
        <PassphrasePrompt
          :key-path="profile?.keyPath ?? ''"
          :endpoint="endpoint"
          :rejected="connection.passphraseRejected"
          :busy="busy"
          @submit="submitPassphrase"
          @cancel="connection.cancel()"
        />
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style scoped>
.banner {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: var(--sp-4);
  padding: var(--sp-3) var(--sp-5);
  background: var(--color-surface);
  border-bottom: var(--border-width) solid var(--color-border);
}

.text {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.title {
  display: flex;
  align-items: baseline;
  gap: var(--sp-2);
  font-size: var(--fs-md);
  font-weight: 600;
}

.detail {
  overflow: hidden;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.failure {
  color: var(--color-danger-text);
  white-space: normal;
}

.code {
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

/* LOS : forme barrée, fond rouge léger, filet rouge. */
.lost {
  background: var(--color-danger-tint);
  border-bottom-color: var(--color-danger);
}

.lost .icon,
.lost .code {
  color: var(--color-danger-text);
}

/* ACQ : ondes, contour pointillé ambre. */
.acquiring,
.passphrase {
  background: var(--color-selection);
  border-bottom: var(--border-width) dashed var(--color-warning);
}

.acquiring .icon,
.acquiring .code,
.passphrase .icon,
.passphrase .code {
  color: var(--color-warning);
}

/* AOS : liaison rétablie, filet vert-de-gris. */
.restored {
  background: var(--color-success-tint);
  border-bottom-color: var(--color-success);
}

.restored .icon,
.restored .code {
  color: var(--color-success);
}

.overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-dialog);
  background: var(--color-scrim);
}

.dialog {
  position: fixed;
  top: 50%;
  left: 50%;
  z-index: var(--z-dialog);
  width: min(calc(100vw - 2 * var(--sp-4)), calc(11 * var(--sp-8)));
  transform: translate(-50%, -50%);
  box-shadow: var(--shadow-overlay);
}
</style>
