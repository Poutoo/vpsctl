import type { ImageFormat } from "./bindings/ImageFormat";

// Images locales affichées par une URL `blob:` (seule source d'image
// autorisée par la CSP en plus des fichiers de l'application).

const MIME: Record<ImageFormat, string> = {
  png: "image/png",
  jpeg: "image/jpeg",
  webp: "image/webp",
};

export function createImageUrl(data: ArrayBuffer, format: ImageFormat): string {
  return URL.createObjectURL(new Blob([data], { type: MIME[format] }));
}

export function revokeImageUrl(url: string | null) {
  if (url !== null) URL.revokeObjectURL(url);
}

/** Décode réellement l'image ; rejeté si le moteur web ne peut pas l'afficher. */
export async function decodeImage(url: string): Promise<void> {
  const image = new Image();
  image.src = url;
  await image.decode();
}
