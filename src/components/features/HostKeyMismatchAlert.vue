<script setup lang="ts">
import { computed } from "vue";
import { useConfirm } from "../../composables/useConfirm";
import AppButton from "../ui/AppButton.vue";
import PanelCard from "../ui/PanelCard.vue";
import StatusIcon from "../ui/StatusIcon.vue";
import FingerprintGroups from "./FingerprintGroups.vue";

const props = defineProps<{
  profileName: string;
  endpoint: string;
  expected: string;
  received: string;
  busy: boolean;
}>();

const emit = defineEmits<{
  back: [];
  replace: [];
}>();

const { confirm } = useConfirm();

function groups(fingerprint: string): string[] {
  return fingerprint.replace(/^SHA256:/, "").match(/.{1,4}/g) ?? [];
}

const comparison = computed(() => {
  const expected = groups(props.expected);
  const received = groups(props.received);
  const identical = received.filter((g, i) => g === expected[i]).length;
  return { identical, total: received.length };
});

/** Remplacement : GO / NO GO, GO actif seulement une fois la vérification cochée. */
async function askReplace() {
  const confirmed = await confirm({
    title: "Remplacer la clé mémorisée ?",
    subject: props.profileName,
    consequences: [
      "La clé mémorisée dans le profil sera remplacée par la clé reçue.",
      "Une nouvelle liaison démarrera aussitôt avec cette clé.",
      "Si la clé reçue vient d'une interception, la liaison passera par elle.",
    ],
    confirmLabel: "Remplacer",
    danger: true,
    acknowledge: "J'ai vérifié l'empreinte reçue sur la console de l'hébergeur",
  });
  if (confirmed) emit("replace");
}
</script>

<template>
  <section class="alert" role="alert" aria-labelledby="mismatch-title">
    <header class="banner">
      <span class="caps kicker">
        <StatusIcon class="shape" name="triangle" />
        Alerte de sécurité · liaison interrompue
      </span>
      <span class="target">{{ profileName }} · {{ endpoint }}</span>
    </header>

    <div class="content">
      <div class="main">
        <h1 id="mismatch-title" class="title">
          L'empreinte du serveur a changé.
        </h1>
        <p class="lead">
          La clé présentée par l'hôte ne correspond pas à celle mémorisée dans
          le profil. Il peut s'agir d'une réinstallation du serveur, ou d'une
          interception. La liaison a été coupée avant l'authentification : aucun
          identifiant n'a été envoyé.
        </p>

        <div class="compare">
          <div class="row">
            <p class="caps row-label">Mémorisée</p>
            <FingerprintGroups :fingerprint="expected" layout="inline" />
          </div>
          <div class="row received">
            <p class="caps row-label">
              Reçue
              <span class="when">à l'instant</span>
            </p>
            <FingerprintGroups
              :fingerprint="received"
              :compare-to="expected"
              layout="inline"
            />
          </div>
          <p class="caps legend">
            <span>SHA256 · groupes différents soulignés</span>
            <span class="count">
              {{ comparison.identical }}
              {{
                comparison.identical > 1
                  ? "groupes identiques"
                  : "groupe identique"
              }}
              sur {{ comparison.total }}
            </span>
          </p>
        </div>
      </div>

      <PanelCard class="procedure">
        <template #kicker>Procédure</template>
        <ol class="steps">
          <li>
            <span class="number">1</span>
            <span>
              Ouvrez la console de l'hébergeur et lisez l'empreinte :
              <code>ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub</code>
            </span>
          </li>
          <li>
            <span class="number">2</span>
            <span>
              Elle correspond à la clé reçue et le serveur a été réinstallé :
              remplacez la clé mémorisée.
            </span>
          </li>
          <li>
            <span class="number">3</span>
            <span>
              Elle ne correspond pas : n'établissez pas la liaison depuis ce
              réseau et vérifiez-le.
            </span>
          </li>
        </ol>
      </PanelCard>
    </div>

    <footer class="actions">
      <AppButton variant="primary" @click="emit('back')">
        Revenir aux profils
      </AppButton>
      <AppButton variant="danger" :disabled="busy" @click="askReplace">
        Remplacer la clé mémorisée…
      </AppButton>
    </footer>
  </section>
</template>

<style scoped>
.alert {
  display: flex;
  flex-direction: column;
  min-height: 100%;
  background: var(--color-bg);
}

.banner {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-6);
  background: var(--color-danger-tint);
  border-bottom: var(--border-width) solid var(--color-danger);
}

.kicker {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  color: var(--color-danger-text);
}

.shape {
  color: var(--color-danger);
}

.target {
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

.content {
  display: flex;
  flex: 1;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: var(--sp-6);
  padding: var(--sp-7) var(--sp-6) var(--sp-5);
}

.main {
  display: flex;
  flex: 2 1 calc(8 * var(--sp-8));
  flex-direction: column;
  gap: var(--sp-4);
  min-width: 0;
}

.title {
  font-size: var(--fs-3xl);
}

.lead {
  max-width: 70ch;
  color: var(--color-text-muted);
}

.compare {
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border-strong);
}

.row {
  display: grid;
  grid-template-columns: calc(2 * var(--sp-8)) minmax(0, 1fr);
  align-items: center;
  gap: var(--sp-4);
  padding: var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
}

.row-label {
  display: flex;
  flex-direction: column;
  color: var(--color-text-muted);
}

.received .row-label {
  color: var(--color-danger-text);
}

.when {
  color: var(--color-text-muted);
  font-family: var(--font-ui);
  letter-spacing: normal;
  text-transform: none;
}

.legend {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: var(--sp-3);
  padding: var(--sp-2) var(--sp-4);
  background: var(--color-surface-raised);
  color: var(--color-text-muted);
}

.count {
  color: var(--color-danger-text);
}

.procedure {
  flex: 1 1 calc(4 * var(--sp-8));
}

.steps {
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: var(--fs-sm);
}

.steps li {
  display: flex;
  gap: var(--sp-4);
  padding: var(--sp-3) 0;
  border-bottom: var(--border-width) solid var(--color-border);
}

.steps li:first-child {
  padding-top: 0;
}

.steps li:last-child {
  padding-bottom: 0;
  border-bottom: none;
}

.number {
  color: var(--color-action);
  font-family: var(--font-mono);
}

code {
  display: block;
  margin-top: var(--sp-2);
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  word-break: break-all;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-3);
  padding: var(--sp-5) var(--sp-6) var(--sp-6);
}
</style>
