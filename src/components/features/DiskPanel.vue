<script setup lang="ts">
/**
 * Espace disque par point de montage (planche VII) : barre d'occupation et
 * repères des deux seuils, espace libre, état. Sous le point de montage de
 * la racine Docker en alerte : répartition de l'espace Docker (automatique,
 * ou « Analyser l'espace Docker » selon `automaticDockerUsage`) et analyse
 * des volumes à la demande. Aucun autre point de montage n'affiche de cause.
 */
import { computed } from "vue";
import type { DiskUsage } from "../../bindings/DiskUsage";
import type { Level } from "../../bindings/Level";
import { formatBytes, formatDuration } from "../../format";
import { formatPercent, formatTime, valueOf } from "../../resourceText";
import { useConnectionStore } from "../../stores/connection";
import { useResourcesStore } from "../../stores/resources";
import AppButton from "../ui/AppButton.vue";
import AppIcon from "../ui/AppIcon.vue";
import StatusIcon from "../ui/StatusIcon.vue";
import ResourceState from "./ResourceState.vue";

const store = useResourcesStore();
const connection = useConnectionStore();

const update = computed(() => store.update);
const thresholds = computed(
  () => store.thresholds ?? update.value?.thresholds ?? null,
);
const disks = computed(() =>
  update.value ? valueOf(update.value.disks) : null,
);
const unavailable = computed(() =>
  update.value?.disks.status === "unavailable"
    ? update.value.disks.reason
    : null,
);

function levelOf(disk: DiskUsage): Level {
  return (
    update.value?.levels.disks.find((d) => d.mountPoint === disk.mountPoint)
      ?.level ?? "nominal"
  );
}

const dockerMount = computed(() => update.value?.dockerMount ?? null);
const dockerAlert = computed(() => {
  const disk = disks.value?.find((d) => d.mountPoint === dockerMount.value);
  return disk && levelOf(disk) !== "nominal" ? disk : null;
});
const automatic = computed(() => update.value?.automaticDockerUsage ?? true);
const dockerUsage = computed(() =>
  automatic.value ? (update.value?.dockerUsage ?? null) : store.usage,
);
const usageCategories = computed(() => {
  const usage = dockerUsage.value;
  if (!usage) return [];
  return [
    { key: "images", label: "Images", bytes: usage.images },
    { key: "containers", label: "Conteneurs", bytes: usage.containers },
    { key: "volumes", label: "Volumes", bytes: usage.volumes },
    { key: "cache", label: "Cache de build", bytes: usage.buildCache },
  ];
});
const dockerSpace = computed(() =>
  dockerAlert.value ? valueOf(dockerAlert.value.space) : null,
);

function share(bytes: number): string {
  const total = dockerSpace.value?.total ?? 0;
  return total > 0 ? `${Math.min(100, (bytes / total) * 100)}%` : "0%";
}

const analysis = computed(() => store.analysis);
const running = computed(() =>
  analysis.value.status === "running" ? analysis.value.kind : null,
);
const failedOf = (kind: "usage" | "volumes") =>
  analysis.value.status === "failed" && analysis.value.kind === kind
    ? analysis.value.message
    : null;
const lostOf = (kind: "usage" | "volumes") =>
  analysis.value.status === "lost" && analysis.value.kind === kind;
const volumes = computed(() => store.volumes);
const largestVolume = computed(() =>
  Math.max(1, ...(volumes.value?.volumes.map((v) => v.size ?? 0) ?? [])),
);
const busy = computed(() => running.value !== null || connection.lost);

function percentWidth(percent: number): string {
  return `${Math.min(100, Math.max(0, percent))}%`;
}
</script>

<template>
  <section class="panel" data-testid="disk-panel">
    <header class="head">
      <h2 class="caps">Espace disque · par point de montage</h2>
      <span v-if="thresholds" class="caps aside"
        >attention {{ thresholds.disk.attention }} % · critique
        {{ thresholds.disk.critical }} %</span
      >
    </header>

    <p v-if="unavailable" class="row unavailable-all">
      <ResourceState state="unavailable" />
      <span>{{ unavailable }}. Les autres mesures restent à jour.</span>
    </p>
    <div v-else-if="!disks" class="row skeleton-row" aria-hidden="true">
      <span class="block point-block" />
      <span class="block bar-block" />
      <ResourceState state="reading" />
    </div>

    <template v-for="disk in disks ?? []" :key="disk.mountPoint">
      <div
        class="row disk"
        :class="levelOf(disk)"
        :data-testid="`disk-${disk.mountPoint}`"
      >
        <div class="mount">
          <span class="point mono">{{ disk.mountPoint }}</span>
          <span class="device mono">{{ disk.device }} · {{ disk.fsType }}</span>
        </div>
        <template v-if="disk.space.status === 'ok'">
          <div class="bar" aria-hidden="true">
            <span
              class="fill"
              :class="levelOf(disk)"
              :style="{ width: percentWidth(disk.space.value.percent) }"
            />
            <template v-if="thresholds">
              <span
                class="mark attention"
                :style="{ left: percentWidth(thresholds.disk.attention) }"
              />
              <span
                class="mark critical"
                :style="{ left: percentWidth(thresholds.disk.critical) }"
              />
            </template>
          </div>
          <div class="usage">
            <span
              class="percent mono"
              :class="levelOf(disk)"
              data-testid="disk-percent"
              >{{ formatPercent(disk.space.value.percent) }}</span
            >
            <span class="free mono"
              >{{ formatBytes(disk.space.value.free) }} libres sur
              {{ formatBytes(disk.space.value.total) }}</span
            >
          </div>
          <ResourceState :state="levelOf(disk)" />
        </template>
        <template v-else>
          <span class="reason">
            {{ disk.space.status === "unavailable" ? disk.space.reason : "" }}
          </span>
          <span />
          <ResourceState state="unavailable" />
        </template>
      </div>

      <!-- Cause probable : uniquement sous le point de montage de Docker. -->
      <div
        v-if="dockerAlert && disk.mountPoint === dockerAlert.mountPoint"
        class="docker"
        :class="levelOf(disk)"
        data-testid="docker-space"
      >
        <div class="docker-box">
          <header class="docker-head">
            <span class="caps">Espace Docker · par catégorie</span>
            <span v-if="dockerSpace" class="mono aside"
              >{{ formatBytes(dockerSpace.total - dockerSpace.free) }} utilisés
              sur {{ formatBytes(dockerSpace.total) }}</span
            >
          </header>

          <template v-if="dockerUsage">
            <div class="stack" aria-hidden="true">
              <span
                v-for="category in usageCategories"
                :key="category.key"
                class="segment"
                :class="category.key"
                :style="{ width: share(category.bytes) }"
              />
            </div>
            <ul class="categories">
              <li
                v-for="category in usageCategories"
                :key="category.key"
                :data-testid="`docker-category-${category.key}`"
              >
                <span class="swatch" :class="category.key" />
                <span>{{ category.label }}</span>
                <span class="mono size">{{ formatBytes(category.bytes) }}</span>
              </li>
            </ul>
            <p class="caps note">
              Répartition de {{ formatTime(dockerUsage.at) }}
            </p>
          </template>
          <p v-else-if="automatic" class="pending">
            <StatusIcon name="reading" /> Répartition en cours de lecture…
          </p>

          <!-- Répartition à la demande (DOCKER_USAGE_AUTOMATIC à false). -->
          <div v-if="!automatic" class="action-row">
            <template v-if="running === 'usage'">
              <span class="caps running"
                ><StatusIcon name="reading" /> Analyse en cours</span
              >
              <span class="hint"
                >{{ dockerMount }} · les relevés continuent.</span
              >
              <AppButton @click="store.cancelAnalysis()">Annuler</AppButton>
            </template>
            <template v-else>
              <AppButton :disabled="busy" @click="store.analyze('usage')">
                <AppIcon name="search" />
                {{
                  dockerUsage
                    ? "Relancer l'analyse de l'espace"
                    : "Analyser l'espace Docker"
                }}
              </AppButton>
              <span v-if="lostOf('usage')" class="hint lost"
                >Liaison perdue · analyse à relancer</span
              >
              <span v-else-if="failedOf('usage')" class="hint error">{{
                failedOf("usage")
              }}</span>
              <span v-else class="hint"
                >Répartition par catégorie (images, conteneurs, volumes, cache
                de build).</span
              >
            </template>
          </div>

          <!-- Analyse des volumes, à la demande. -->
          <div
            v-if="running === 'volumes'"
            class="action-row running-row"
            data-testid="volumes-running"
          >
            <span class="caps running"
              ><StatusIcon name="reading" /> Analyse en cours</span
            >
            <span class="hint"
              >{{ dockerMount }} · peut durer plusieurs minutes. Les relevés
              continuent.</span
            >
            <AppButton
              data-testid="volumes-cancel"
              @click="store.cancelAnalysis()"
              >Annuler</AppButton
            >
          </div>
          <template v-else-if="volumes && !lostOf('volumes')">
            <header class="docker-head volumes-head">
              <span class="caps"
                >Volumes · analyse de {{ formatTime(volumes.at) }} ·
                {{ formatDuration(volumes.durationMs) }}</span
              >
              <AppButton
                :disabled="busy"
                data-testid="volumes-analyze"
                @click="store.analyze('volumes')"
                >Relancer l'analyse</AppButton
              >
            </header>
            <p v-if="failedOf('volumes')" class="hint error">
              {{ failedOf("volumes") }}
            </p>
            <ul class="volumes" data-testid="volumes-list">
              <li v-for="volume in volumes.volumes" :key="volume.name">
                <span class="volume-name">
                  <span class="mono">{{ volume.name }}</span>
                  <span v-if="volume.containers.length > 0" class="users">
                    · {{ volume.containers.join(", ") }}</span
                  >
                </span>
                <span class="volume-bar" aria-hidden="true">
                  <span
                    class="fill"
                    :style="{
                      width: `${((volume.size ?? 0) / largestVolume) * 100}%`,
                    }"
                  />
                </span>
                <span class="mono size">{{
                  volume.size === null ? "N/A" : formatBytes(volume.size)
                }}</span>
              </li>
              <li v-if="volumes.volumes.length === 0" class="hint">
                Aucun volume.
              </li>
            </ul>
          </template>
          <div v-else class="action-row">
            <AppButton
              :disabled="busy"
              data-testid="volumes-analyze"
              @click="store.analyze('volumes')"
            >
              <AppIcon name="search" />
              {{
                lostOf("volumes")
                  ? "Relancer l'analyse"
                  : "Analyser les volumes"
              }}
            </AppButton>
            <span
              v-if="lostOf('volumes')"
              class="hint lost"
              data-testid="volumes-lost"
              >Liaison perdue · analyse à relancer</span
            >
            <span v-else-if="failedOf('volumes')" class="hint error">{{
              failedOf("volumes")
            }}</span>
            <span v-else class="hint"
              >Détail par volume. Lecture lourde sur le serveur, à lancer à la
              demande.</span
            >
          </div>
        </div>
      </div>
    </template>

    <footer class="legend caps">
      <span class="key"><span class="tick attention" />seuil d'attention</span>
      <span class="key"><span class="tick critical" />seuil critique</span>
    </footer>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.head,
.docker-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
}

.head {
  min-height: var(--row-height);
  padding: var(--sp-2) var(--sp-4);
  background: var(--color-surface-raised);
  border-bottom: var(--border-width) solid var(--color-border);
}

.head h2 {
  color: var(--color-text-muted);
}

.aside {
  color: var(--color-text-muted);
}

.row {
  display: grid;
  grid-template-columns: minmax(0, 3fr) minmax(0, 6fr) minmax(0, 3fr) minmax(
      0,
      2fr
    );
  align-items: center;
  gap: var(--sp-4);
  padding: var(--sp-3) var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
  box-shadow: inset var(--border-width-accent) 0 0 transparent;
}

.disk.attention {
  background: var(--color-selection);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-warning);
}

.disk.critical {
  background: var(--color-danger-tint);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-danger);
}

.unavailable-all {
  display: flex;
  gap: var(--sp-3);
  color: var(--color-text-muted);
}

.mount {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.point {
  overflow: hidden;
  font-size: var(--fs-md);
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.device {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.bar,
.volume-bar {
  position: relative;
  height: var(--sp-2);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border-strong);
}

.fill {
  position: absolute;
  inset: 0 auto 0 0;
  background: var(--color-text-muted);
}

.fill.attention {
  background: var(--color-warning);
}

.fill.critical {
  background: var(--color-danger);
}

.mark {
  position: absolute;
  top: calc(-1 * var(--sp-1));
  bottom: calc(-1 * var(--sp-1));
  border-left: var(--border-width) dashed var(--color-warning);
}

.mark.critical {
  border-left: var(--border-width) dotted var(--color-danger-text);
}

.usage {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: var(--sp-1);
}

.percent {
  font-size: var(--fs-lg);
  font-weight: 600;
}

.percent.attention {
  color: var(--color-warning);
}

.percent.critical {
  color: var(--color-danger-text);
}

.free {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
  white-space: nowrap;
}

.reason {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.skeleton-row .block {
  display: block;
  height: var(--sp-2);
  background: var(--color-surface-hover);
}

.docker {
  padding: 0 var(--sp-4) var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
}

.docker.attention {
  background: var(--color-selection);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-warning);
}

.docker.critical {
  background: var(--color-danger-tint);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-danger);
}

.docker-box {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-4);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border);
}

.docker-head .caps {
  color: var(--color-text-muted);
}

.stack {
  display: flex;
  height: var(--sp-3);
  background: var(--color-surface);
}

.segment.images,
.swatch.images {
  background: var(--color-text);
}

.segment.containers,
.swatch.containers {
  background: var(--color-text-muted);
}

.segment.volumes,
.swatch.volumes {
  background: var(--color-text-subtle);
}

.segment.cache,
.swatch.cache {
  background: var(--color-border-strong);
}

.segment + .segment {
  border-left: var(--border-width) solid var(--color-field);
}

.categories {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--sp-4);
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: var(--fs-sm);
}

.categories li {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.swatch {
  display: inline-block;
  width: var(--sp-2);
  height: var(--sp-2);
}

.size {
  margin-left: auto;
  white-space: nowrap;
}

.note {
  color: var(--color-text-subtle);
}

.pending,
.running {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  color: var(--color-text-muted);
}

.action-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-3);
  padding-top: var(--sp-3);
  border-top: var(--border-width) solid var(--color-border);
}

.running-row {
  border-top-style: dashed;
}

.hint {
  flex: 1;
  min-width: 0;
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.hint.lost {
  color: var(--color-warning);
}

.hint.error {
  color: var(--color-danger-text);
}

.volumes-head {
  padding-top: var(--sp-3);
  border-top: var(--border-width) solid var(--color-border);
}

.volumes {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.volumes li {
  display: grid;
  grid-template-columns: minmax(0, 3fr) minmax(0, 4fr) minmax(0, 1fr);
  align-items: center;
  gap: var(--sp-4);
}

.volume-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.users {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.legend {
  display: flex;
  gap: var(--sp-5);
  padding: var(--sp-2) var(--sp-4);
  color: var(--color-text-muted);
  text-transform: none;
}

.key {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
}

.tick {
  display: inline-block;
  height: var(--sp-3);
  border-left: var(--border-width) dashed var(--color-warning);
}

.tick.critical {
  border-left: var(--border-width) dotted var(--color-danger-text);
}

@media (max-width: 1100px) {
  .categories {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
