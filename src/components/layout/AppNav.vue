<script setup lang="ts">
/**
 * Navigation principale, affichée pendant une liaison. Section active : trait
 * d'action de 2 px, numéral en ambre, texte plein. En fenêtre étroite, seuls
 * les numéraux restent, chacun avec une infobulle.
 */
import { computed } from "vue";
import { useRoute } from "vue-router";
import { formatClock, useElapsed } from "../../composables/useElapsed";
import { NARROW_QUERY, useMediaQuery } from "../../composables/useMediaQuery";
import { SECTIONS, sectionOf } from "../../navigation";
import { useConnectionStore } from "../../stores/connection";
import { useFilesStore } from "../../stores/files";
import { useScriptGuards } from "../../composables/useScriptGuards";
import { useProfilesStore } from "../../stores/profiles";
import { streamOrigin, useTelemetryStore } from "../../stores/telemetry";
import AppButton from "../ui/AppButton.vue";
import AppIcon from "../ui/AppIcon.vue";
import AppTooltip from "../ui/AppTooltip.vue";
import TelemetryIndicator from "./TelemetryIndicator.vue";

const route = useRoute();
const connection = useConnectionStore();
const profiles = useProfilesStore();
const files = useFilesStore();
const telemetry = useTelemetryStore();
const { confirmLeave } = useScriptGuards();
const narrow = useMediaQuery(NARROW_QUERY);

const profile = computed(() => profiles.byId(connection.profileId));

/** Section active ; sur l'écran Télémétrie, celle d'où vient le flux affiché. */
const active = computed(() => {
  if (route.name === "telemetry") {
    const tab = telemetry.tabById(String(route.params.tab ?? ""));
    return tab ? streamOrigin(tab) : null;
  }
  return route.meta.section ?? sectionOf(route.name)?.route ?? null;
});

const connectedAt = computed(() => connection.info?.connectedAt ?? null);
const linkedFor = useElapsed(connectedAt, {
  clock: () => Date.now(),
  tickMs: 1_000,
});

/** Scripts en cours : GO / NO GO, ils seront interrompus. */
async function disconnect() {
  if (!(await confirmLeave("disconnect"))) return;
  await connection.disconnect();
}
</script>

<template>
  <nav class="nav" :class="{ narrow }" aria-label="Navigation principale">
    <!-- En fenêtre étroite, le profil reste lisible dans l'indicateur AOS. -->
    <div v-if="profile && !narrow" class="profile">
      <p class="name">{{ profile.name }}</p>
      <p class="endpoint">{{ profile.user }}@{{ profile.host }}</p>
    </div>

    <ul class="sections">
      <li v-for="section in SECTIONS" :key="section.route">
        <AppTooltip :text="section.label" side="right" :disabled="!narrow">
          <RouterLink
            :to="
              section.route === 'files'
                ? files.entryRoute
                : { name: section.route }
            "
            class="item"
            :class="{ active: active === section.route }"
            :aria-current="active === section.route ? 'page' : undefined"
          >
            <span class="numeral" aria-hidden="true">{{
              section.numeral
            }}</span>
            <span :class="{ 'visually-hidden': narrow }">{{
              section.label
            }}</span>
          </RouterLink>
        </AppTooltip>
      </li>
    </ul>

    <div class="footer">
      <TelemetryIndicator :narrow="narrow" />
      <p v-if="linkedFor !== null && !narrow" class="since mono">
        Liaison depuis {{ formatClock(linkedFor) }}
      </p>
      <AppTooltip v-if="narrow" text="Couper la liaison" side="right">
        <AppButton
          class="icon-button"
          aria-label="Couper la liaison"
          @click="disconnect"
        >
          <AppIcon name="signal-lost" />
        </AppButton>
      </AppTooltip>
      <AppButton v-else @click="disconnect">Couper la liaison</AppButton>
    </div>
  </nav>
</template>

<style scoped>
.nav {
  display: flex;
  flex-direction: column;
  width: var(--nav-width);
  min-height: 0;
  background: var(--color-surface);
  border-right: var(--border-width) solid var(--color-border);
}

.nav.narrow {
  width: var(--nav-width-compact);
}

.profile {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  padding: var(--sp-5) var(--sp-4) var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
}

.name {
  overflow: hidden;
  font-family: var(--font-title);
  font-size: var(--fs-xl);
  line-height: var(--lh-title);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.endpoint {
  overflow: hidden;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sections {
  display: flex;
  flex: 1;
  flex-direction: column;
  margin: 0;
  padding: var(--sp-2) 0;
  list-style: none;
}

.item {
  display: flex;
  align-items: baseline;
  gap: var(--sp-3);
  padding: var(--sp-2) var(--sp-4);
  color: var(--color-text-muted);
  font-size: var(--fs-md);
  font-weight: 600;
  text-decoration: none;
  box-shadow: inset var(--border-width-accent) 0 0 transparent;
  transition:
    background-color var(--dur-fast) var(--ease-out),
    color var(--dur-fast) var(--ease-out);
}

.narrow .item {
  justify-content: center;
  padding-inline: 0;
}

.item:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.item:focus-visible {
  outline-offset: calc(-1 * var(--focus-offset));
}

.item.active {
  background: var(--color-surface-raised);
  color: var(--color-text);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-action);
}

/* Numéraux romains : EB Garamond, alignés sur une colonne. */
.numeral {
  min-width: 3ch;
  color: var(--color-text-muted);
  font-family: var(--font-title);
  font-size: var(--fs-lg);
  font-weight: 400;
}

.narrow .numeral {
  min-width: 0;
}

.active .numeral {
  color: var(--color-action);
}

.footer {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--sp-3);
  padding: var(--sp-4);
  border-top: var(--border-width) solid var(--color-border);
}

.narrow .footer {
  align-items: center;
  padding-inline: 0;
}

.since {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.icon-button {
  padding-inline: var(--sp-2);
}
</style>
