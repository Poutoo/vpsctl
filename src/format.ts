/** Formats d'affichage partagés (tailles, dates), en français. */

const BYTE_UNITS = ["o", "Ko", "Mo", "Go", "To"] as const;

/**
 * Taille lisible en base 1024 : « 418 o », « 1,2 Ko », « 128 Mo ». Avec
 * `fixed`, toujours une décimale au-delà de l'octet (« 18,0 Mo »), pour
 * qu'une valeur qui progresse ne change pas de largeur.
 */
export function formatBytes(bytes: number, fixed = false): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < BYTE_UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const decimals = unit === 0 ? 0 : fixed || value < 10 ? 1 : 0;
  const text = value.toLocaleString("fr-FR", {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
  return `${text} ${BYTE_UNITS[unit]}`;
}

const DAY_MONTH = new Intl.DateTimeFormat("fr-FR", {
  day: "numeric",
  month: "short",
});
const DAY_MONTH_YEAR = new Intl.DateTimeFormat("fr-FR", {
  day: "numeric",
  month: "short",
  year: "numeric",
});
const HOUR_MINUTE = new Intl.DateTimeFormat("fr-FR", {
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});

/**
 * Date de modification : « 12 oct. 14:02 » pour l'année en cours,
 * « 12 oct. 2025 » sinon.
 */
export function formatModified(millis: number, now = new Date()): string {
  const date = new Date(millis);
  if (date.getFullYear() === now.getFullYear()) {
    return `${DAY_MONTH.format(date)} ${HOUR_MINUTE.format(date)}`;
  }
  return DAY_MONTH_YEAR.format(date);
}

/** Durée en minutes et secondes : « 00:14 », « 72:05 ». */
export function formatClock(seconds: number): string {
  const total = Math.max(0, Math.round(seconds));
  const minutes = Math.floor(total / 60);
  const rest = total % 60;
  return `${String(minutes).padStart(2, "0")}:${String(rest).padStart(2, "0")}`;
}
