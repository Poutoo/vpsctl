import type { BackgroundInfo } from "./bindings/BackgroundInfo";
import type { Refusal } from "./bindings/Refusal";
import { formatBytes } from "./format";

// Textes de l'image de fond (planche VI). Les limites sont celles du
// backend (`background.rs`), rappelées ici pour les messages.

const MAX_SIDE = 8192;
const MAX_MEGAPIXELS = 40;
const KEPT = "Le fond actuel est conservé.";

const number = (value: number) => value.toLocaleString("fr-FR");

/** Taille et dimensions, ex. « 2,4 Mo · 3840 × 2160 ». */
export function backgroundDetail(info: BackgroundInfo): string {
  return `${formatBytes(info.bytes, true)} · ${info.width} × ${info.height}`;
}

export function refusalTitle(refusal: Refusal): string {
  switch (refusal.kind) {
    case "format":
      return "Image refusée · format non pris en charge";
    case "tooLarge":
      return "Image refusée · fichier trop volumineux";
    case "dimensions":
      return "Image refusée · dimensions trop grandes";
    case "unreadable":
      return "Image refusée · image illisible";
  }
}

export function refusalText(fileName: string, refusal: Refusal): string {
  const name = `« ${fileName} »`;
  switch (refusal.kind) {
    case "format":
      return `${name} n'est pas lisible. Formats acceptés : PNG, JPEG et WebP. ${KEPT}`;
    case "tooLarge":
      return `${name} pèse ${formatBytes(refusal.bytes, true)} ; la limite est de 10 Mo. ${KEPT}`;
    case "dimensions":
      return `${name} mesure ${number(refusal.width)} × ${number(refusal.height)} px ; la limite est de ${number(MAX_SIDE)} px de côté et ${MAX_MEGAPIXELS} mégapixels. ${KEPT}`;
    case "unreadable":
      return `${name} ne peut pas être décodée : le fichier est abîmé ou incomplet. ${KEPT}`;
  }
}
