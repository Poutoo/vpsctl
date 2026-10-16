<script setup lang="ts">
/**
 * File d'attente des transferts (« Transferts · Orbites ») : un transfert
 * actif à la fois, les autres en attente. États : en attente, en cours,
 * terminé, annulé, en erreur ; jamais par la couleur seule (forme et
 * libellé). Actions : Annuler, Retirer, Réessayer, et Afficher pour un
 * téléchargement terminé.
 */
import { computed, ref } from "vue";
import { errorMessage } from "../../api/errors";
import { formatBytes, formatClock } from "../../format";
import { useTransferActions } from "../../composables/useTransferActions";
import { useTransfersStore, type Transfer } from "../../stores/transfers";
import AppButton from "../ui/AppButton.vue";
import IconButton from "../ui/IconButton.vue";
import StatusIcon from "../ui/StatusIcon.vue";
import TransferOrbit from "./TransferOrbit.vue";

const transfers = useTransfersStore();
const { server, retry } = useTransferActions();

const summary = computed(() => {
  const { active, queued, error } = transfers.counts;
  const parts: string[] = [];
  if (active > 0) parts.push(`${active} en cours`);
  if (queued > 0) parts.push(`${queued} en attente`);
  if (error > 0) parts.push(`${error} en erreur`);
  return parts.length > 0 ? parts.join(" · ") : "aucun en cours";
});

function progressOf(item: Transfer): number {
  if (item.status === "done") return 1;
  return item.total ? item.transferred / item.total : 0;
}

function percent(item: Transfer): string {
  return `${Math.floor(progressOf(item) * 100)} %`;
}

function route(item: Transfer): string {
  return item.direction === "upload"
    ? `Local → ${server.value} · Envoi`
    : `${server.value} → Local · Réception`;
}

const SHAPES = {
  queued: "circle",
  active: "half",
  done: "check",
  cancelled: "circle",
  error: "cross",
} as const;

function statusText(item: Transfer): string {
  switch (item.status) {
    case "queued":
      return "En attente";
    case "active":
      return percent(item);
    case "done":
      return item.direction === "upload" ? "Envoyé" : "Reçu";
    case "cancelled":
      return "Annulé";
    case "error":
      return `${percent(item)} · Erreur`;
  }
}

function detail(item: Transfer): string {
  const total = item.total === null ? null : formatBytes(item.total, true);
  switch (item.status) {
    case "queued":
      return "—";
    case "active": {
      const parts = [
        total === null
          ? formatBytes(item.transferred, true)
          : `${formatBytes(item.transferred, true)} / ${total}`,
      ];
      if (item.rate !== null && item.rate > 0) {
        parts.push(`${formatBytes(item.rate, true)}/s`);
        if (item.total !== null) {
          const remaining = (item.total - item.transferred) / item.rate;
          parts.push(`reste ${formatClock(remaining)}`);
        }
      }
      return parts.join(" · ");
    }
    case "done": {
      const duration =
        item.startedAt !== null && item.finishedAt !== null
          ? ` · ${formatClock((item.finishedAt - item.startedAt) / 1000)}`
          : "";
      return `${formatBytes(item.total ?? item.transferred)}${duration}`;
    }
    case "cancelled":
      return item.transferred > 0
        ? `Arrêté à ${formatBytes(item.transferred)}`
        : "Arrêté avant le début";
    case "error":
      return total === null
        ? "LOS"
        : `LOS à ${formatBytes(item.transferred)} sur ${total}`;
  }
}

/** Annulations en cours : début du chrono T+ du bouton. */
const cancelling = ref<Record<number, number>>({});

async function cancel(item: Transfer) {
  cancelling.value = { ...cancelling.value, [item.key]: performance.now() };
  try {
    await transfers.cancel(item.key);
  } finally {
    const rest = { ...cancelling.value };
    delete rest[item.key];
    cancelling.value = rest;
  }
}
</script>

<template>
  <section class="transfers" aria-labelledby="transfers-title">
    <header class="banner">
      <h2 id="transfers-title" class="caps kicker">Transferts · Orbites</h2>
      <span class="aside">{{ summary }}</span>
    </header>
    <ul class="list">
      <li
        v-for="item in transfers.items"
        :key="item.key"
        class="transfer"
        :class="item.status"
      >
        <div class="identity">
          <span class="name" :title="item.remotePath">{{ item.name }}</span>
          <span class="caps route">{{ route(item) }}</span>
        </div>
        <TransferOrbit
          :direction="item.direction"
          :status="item.status"
          :progress="progressOf(item)"
          :server="server"
        />
        <div class="state">
          <span class="caps label">
            <StatusIcon class="shape" :name="SHAPES[item.status]" />
            {{ statusText(item) }}
          </span>
          <span class="detail" :title="detail(item)">{{ detail(item) }}</span>
          <span
            v-if="item.status === 'error' && item.error"
            class="message"
            :title="errorMessage(item.error)"
            >{{ errorMessage(item.error) }}</span
          >
        </div>
        <div class="actions">
          <AppButton
            v-if="item.status === 'active'"
            :busy-since="cancelling[item.key] ?? null"
            :disabled="cancelling[item.key] !== undefined"
            @click="cancel(item)"
          >
            Annuler
          </AppButton>
          <AppButton
            v-else-if="item.status === 'queued'"
            @click="transfers.remove(item.key)"
          >
            Retirer
          </AppButton>
          <template v-else>
            <AppButton
              v-if="item.status === 'done' && item.direction === 'download'"
              @click="transfers.reveal(item.key)"
            >
              Afficher
            </AppButton>
            <AppButton
              v-else-if="item.status === 'error' || item.status === 'cancelled'"
              @click="retry(item)"
            >
              Réessayer
            </AppButton>
            <IconButton
              icon="close"
              :label="`Retirer ${item.name} de la file`"
              @click="transfers.remove(item.key)"
            />
          </template>
        </div>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.transfers {
  flex-shrink: 0;
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  min-height: var(--row-height);
  padding: var(--sp-2) var(--sp-4);
  background: var(--color-surface-raised);
  border-bottom: var(--border-width) solid var(--color-border);
}

.kicker {
  margin: 0;
  color: var(--color-text-muted);
}

.aside {
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

.list {
  max-height: var(--transfer-list-height);
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
}

.transfer {
  display: grid;
  grid-template-columns:
    minmax(0, 1.2fr) var(--orbit-width) minmax(0, 1fr)
    auto;
  align-items: center;
  gap: var(--sp-5);
  padding: var(--sp-3) var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
}

.transfer:last-child {
  border-bottom: none;
}

/* Erreur : fond teinté (l'état reste porté par sa forme et son libellé). */
.transfer.error {
  background: var(--color-danger-tint);
}

.identity,
.state {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.name {
  overflow: hidden;
  font-family: var(--font-mono);
  font-size: var(--fs-data);
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.route {
  color: var(--color-text-muted);
}

.label {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  color: var(--color-text);
}

.queued .label,
.cancelled .label {
  color: var(--color-text-muted);
}

.active .shape {
  color: var(--color-warning);
}

.done .shape {
  color: var(--color-success);
}

.error .shape {
  color: var(--color-danger);
}

.error .label {
  color: var(--color-danger-text);
}

/* Détail (taille, débit, temps restant) : passe à la ligne plutôt que d'être coupé. */
.detail {
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

.message {
  overflow: hidden;
  color: var(--color-danger-text);
  font-size: var(--fs-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--sp-2);
}
</style>
