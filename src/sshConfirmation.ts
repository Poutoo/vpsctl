/**
 * GO / NO GO de l'enregistrement de `~/.ssh/authorized_keys` (maquettes 09
 * à 11), d'après les vérifications du backend (fonction pure). La case
 * « autre moyen d'accès » est toujours requise ; la fenêtre la décoche à
 * chaque ouverture.
 */
import type { AuthorizedKeysCheck } from "./bindings/AuthorizedKeysCheck";
import type { ConfirmItem, ConfirmOptions } from "./composables/useConfirm";
import { lineRanges } from "./editorDiff";

function plural(count: number, singular: string, pluralForm: string): string {
  return `${count} ${count > 1 ? pluralForm : singular}`;
}

/** « ED25519 · deploy@poste · …V5gT0hPq ». */
function linkKeyLabel(check: AuthorizedKeysCheck): string {
  const key = check.linkKey;
  if (!key) return "clé inconnue";
  return [key.algorithm, key.comment, key.tail].filter(Boolean).join(" · ");
}

export type SshState = "kept" | "removed" | "absent";

export function sshState(check: AuthorizedKeysCheck): SshState {
  if (check.before !== null && check.after === null) return "removed";
  if (check.before === null && check.after === null) return "absent";
  return "kept";
}

export function sshConfirmation(
  check: AuthorizedKeysCheck,
  account: string,
): ConfirmOptions {
  const state = sshState(check);
  const items: ConfirmItem[] = [];
  if (state === "removed") {
    items.push({
      tone: "danger",
      label: "La clé de cette liaison est retirée du fichier",
      detail: `${linkKeyLabel(check)} · ligne ${check.before?.line} à l'ouverture`,
    });
  } else if (state === "absent") {
    items.push({
      tone: "neutral",
      label: "Clé de cette liaison introuvable avant comme après",
      detail:
        "Absente à l'ouverture et dans votre version : cette modification ne change rien pour elle. La liaison utilise peut-être un autre fichier ou une autre clé.",
    });
  } else if (check.after) {
    const options = check.after.options.length
      ? ` · options : ${check.after.options.join(", ")}`
      : "";
    items.push({
      tone: "ok",
      label: `La clé de cette liaison reste autorisée · ligne ${check.after.line}`,
      detail: `${linkKeyLabel(check)}${options}`,
    });
  }

  const total = check.keys + check.certificates + check.authorities;
  const parts: string[] = [];
  if (check.certificates > 0) {
    parts.push(
      `dont ${plural(check.certificates, "certificat", "certificats")}`,
    );
  }
  if (check.authorities > 0) {
    parts.push(
      plural(
        check.authorities,
        "autorité de certification",
        "autorités de certification",
      ),
    );
  }
  if (check.unrecognized.length > 0) {
    const several = check.unrecognized.length > 1;
    parts.push(
      `${several ? "lignes" : "ligne"} ${lineRanges(check.unrecognized)} non ${several ? "reconnues" : "reconnue"} par Atlas`,
    );
  }
  items.push({
    tone: check.unrecognized.length > 0 ? "warn" : "ok",
    label: plural(total, "clé reconnue", "clés reconnues"),
    detail: parts.join(" · ") || undefined,
  });
  if (check.added.length > 0) {
    items.push({
      tone: "warn",
      label: plural(check.added.length, "clé ajoutée", "clés ajoutées"),
      detail: check.added.map((k) => `+ ${k}`).join(" · "),
    });
  }
  if (check.removed.length > 0) {
    items.push({
      tone: "warn",
      label: plural(check.removed.length, "clé retirée", "clés retirées"),
      detail: check.removed.map((k) => `− ${k}`).join(" · "),
    });
  }
  const mode = check.mode ?? "?";
  items.push(
    check.tooOpen
      ? {
          tone: "danger",
          label: "Droits trop ouverts, conservés tels quels",
          detail: `${mode} · sshd refuse un fichier modifiable par le groupe ou les autres`,
        }
      : {
          tone: "ok",
          label: "Droits conservés",
          detail: `${mode} · sshd refuse un fichier trop ouvert`,
        },
  );

  const removed = state === "removed";
  return {
    kicker: "Accès SSH · Confirmation requise",
    aside: removed
      ? "Perte d'accès probable"
      : state === "absent"
        ? "Clé absente avant et après"
        : "Accès conservé",
    title: removed
      ? "Enregistrer sans la clé de cette liaison ?"
      : "Enregistrer un fichier d'accès SSH ?",
    subject: "~/.ssh/authorized_keys",
    command: `Lu par sshd à chaque nouvelle connexion de ${account}`,
    alert: removed
      ? {
          label: "Liaison en cours menacée",
          text: "La liaison en cours reste ouverte jusqu'à sa fermeture, mais Atlas ne pourra plus se reconnecter avec cette clé. Sans autre accès, vous serez enfermé dehors.",
        }
      : undefined,
    items,
    consequences: [],
    acknowledge: removed
      ? "J'ai un autre moyen d'accéder au serveur : cette modification retire la clé de cette liaison"
      : "J'ai un autre moyen d'accéder au serveur si cette modification me bloque",
    acknowledgeHint: removed
      ? "Console de l'hébergeur, ou seconde session SSH ouverte avec une autre clé. Sans cela, ne cochez pas."
      : "Console de l'hébergeur ou seconde session SSH ouverte. La liaison en cours n'est pas coupée, les suivantes peuvent l'être.",
    acknowledgeDanger: removed,
    confirmLabel: "Enregistrer",
    danger: true,
  };
}
