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
  const decimalsFor = (shown: number) =>
    unit === 0 ? 0 : fixed || shown < 10 ? 1 : 0;
  // L'unité et les décimales se décident sur la valeur arrondie :
  // 1 023,96 Ko s'affiche « 1,0 Mo », pas « 1 024 Ko » ; 9,96 Ko, « 10 Ko ».
  let rounded = Number(value.toFixed(decimalsFor(value)));
  if (rounded >= 1024 && unit < BYTE_UNITS.length - 1) {
    unit += 1;
    rounded = Number((rounded / 1024).toFixed(decimalsFor(rounded / 1024)));
  }
  const decimals = decimalsFor(rounded);
  rounded = Number(rounded.toFixed(decimals));
  const text = rounded.toLocaleString("fr-FR", {
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

/**
 * Ancienneté courte : « à l'instant », « il y a 5 min », « il y a 9 h »,
 * « il y a 2 j » ; au-delà de 30 jours, la date (`formatModified`).
 */
export function formatAgo(millis: number, now = Date.now()): string {
  const seconds = Math.max(0, Math.floor((now - millis) / 1000));
  if (seconds < 60) return "à l'instant";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `il y a ${minutes} min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `il y a ${hours} h`;
  const days = Math.floor(hours / 24);
  if (days <= 30) return `il y a ${days} j`;
  return `le ${formatModified(millis, new Date(now))}`;
}
