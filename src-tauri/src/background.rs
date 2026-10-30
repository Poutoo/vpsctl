//! Image de fond personnalisée : contrôles d'une image choisie par
//! l'utilisateur, avant sa copie dans le dossier de configuration.
//!
//! Fonctions pures, dans cet ordre : taille du fichier, format reconnu sur
//! ses premiers octets (jamais sur l'extension), dimensions lues dans
//! l'en-tête, fin du fichier présente (un fichier tronqué serait décodé en
//! partie par le moteur web, sans erreur). Le décodage complet n'a pas lieu ici : la webview s'en charge
//! (`img.decode()`) avant que la copie soit enregistrée.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Taille maximale du fichier : 10 Mo (unités binaires, comme l'affichage).
pub const MAX_BYTES: u64 = 10 * 1024 * 1024;
/// Plus grand côté accepté, en pixels.
pub const MAX_SIDE: u32 = 8192;
/// Surface maximale : environ 160 Mo une fois décodée en RGBA.
pub const MAX_PIXELS: u64 = 40_000_000;
/// Longueur maximale du nom affiché, en caractères.
const MAX_NAME_CHARS: usize = 128;
/// Fin de fichier où chercher le bloc `IEND` d'un PNG.
const TAIL_BYTES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
}

/// Image de fond enregistrée. Le chemin d'origine n'est jamais conservé :
/// seul son dernier composant, pour l'affichage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackgroundInfo {
    pub file_name: String,
    pub format: ImageFormat,
    #[ts(type = "number")]
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
}

/// Raison du refus d'une image. Le fond actuel est conservé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Refusal {
    /// Ni PNG, ni JPEG, ni WebP d'après les premiers octets.
    Format,
    TooLarge {
        #[ts(type = "number")]
        bytes: u64,
    },
    /// Dimensions au-delà de `MAX_SIDE` ou de `MAX_PIXELS`.
    Dimensions { width: u32, height: u32 },
    /// En-tête incohérent, ou décodage refusé par la webview.
    Unreadable,
}

/// Résultat de l'examen d'un fichier : format et dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probe {
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
}

/// Contrôle de la taille, à faire avant de lire le fichier.
pub fn check_size(bytes: u64) -> Result<(), Refusal> {
    if bytes > MAX_BYTES {
        Err(Refusal::TooLarge { bytes })
    } else {
        Ok(())
    }
}

/// Format, puis dimensions et leurs limites.
pub fn inspect(data: &[u8]) -> Result<Probe, Refusal> {
    check_size(data.len() as u64)?;
    let format = sniff(data).ok_or(Refusal::Format)?;
    let (width, height) = match format {
        ImageFormat::Png => png_size(data).filter(|_| png_complete(data)),
        ImageFormat::Jpeg => jpeg_size(data),
        ImageFormat::Webp => webp_size(data).filter(|_| webp_complete(data)),
    }
    .filter(|(w, h)| *w > 0 && *h > 0)
    .ok_or(Refusal::Unreadable)?;
    if width > MAX_SIDE || height > MAX_SIDE || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(Refusal::Dimensions { width, height });
    }
    Ok(Probe {
        format,
        width,
        height,
    })
}

/// Format d'après les premiers octets.
pub fn sniff(data: &[u8]) -> Option<ImageFormat> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(ImageFormat::Png)
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(ImageFormat::Jpeg)
    } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some(ImageFormat::Webp)
    } else {
        None
    }
}

/// Nom affiché : dernier composant du chemin, sans caractère de contrôle,
/// 128 caractères au plus.
pub fn display_name(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let clean: String = name
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect();
    if clean.trim().is_empty() {
        "image".to_owned()
    } else {
        clean
    }
}

fn be_u32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn be_u16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn le_u16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn le_u24(data: &[u8], at: usize) -> Option<u32> {
    let b = data.get(at..at + 3)?;
    Some(u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16)
}

/// PNG : le premier bloc doit être `IHDR`, de 13 octets.
fn png_size(data: &[u8]) -> Option<(u32, u32)> {
    if be_u32(data, 8)? != 13 || data.get(12..16)? != b"IHDR" {
        return None;
    }
    Some((be_u32(data, 16)?, be_u32(data, 20)?))
}

/// Bloc final `IEND` (vide) présent vers la fin du fichier ; quelques
/// octets superflus après lui sont tolérés.
fn png_complete(data: &[u8]) -> bool {
    let tail = &data[data.len().saturating_sub(TAIL_BYTES)..];
    tail.windows(8).any(|w| w == b"\0\0\0\0IEND")
}

/// Taille annoncée par l'en-tête RIFF disponible en entier.
fn webp_complete(data: &[u8]) -> bool {
    data.get(4..8)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
        .is_some_and(|size| (size as usize).saturating_add(8) <= data.len())
}

/// JPEG : parcours des segments jusqu'au premier marqueur SOFn, puis
/// marqueur de fin `FF D9` présent après lui (celui d'une vignette EXIF,
/// placée avant, ne compte pas : dans les données compressées, un 0xFF est
/// toujours suivi de 0x00 ou d'un marqueur RSTn).
fn jpeg_size(data: &[u8]) -> Option<(u32, u32)> {
    let mut at = 2;
    loop {
        if *data.get(at)? != 0xFF {
            return None;
        }
        // Octets de bourrage : une suite de 0xFF avant le marqueur.
        while *data.get(at + 1)? == 0xFF {
            at += 1;
        }
        let marker = *data.get(at + 1)?;
        at += 2;
        match marker {
            // Marqueurs sans longueur (RSTn, TEM) ; SOI répété ou EOI : invalide.
            0xD0..=0xD7 | 0x01 => continue,
            0xD8..=0xDA => return None,
            _ => {}
        }
        let length = usize::from(be_u16(data, at)?);
        if length < 2 {
            return None;
        }
        // SOFn, sauf DHT (C4), JPG (C8) et DAC (CC).
        if matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            let height = be_u16(data, at + 3)?;
            let width = be_u16(data, at + 5)?;
            let ended = data.get(at..)?.windows(2).any(|w| w == [0xFF, 0xD9]);
            return ended.then_some((u32::from(width), u32::from(height)));
        }
        at += length;
    }
}

/// WebP : premier bloc `VP8 ` (avec perte), `VP8L` (sans perte) ou `VP8X`
/// (étendu).
fn webp_size(data: &[u8]) -> Option<(u32, u32)> {
    match data.get(12..16)? {
        b"VP8 " => {
            // Image clé : code de synchronisation 9D 01 2A, puis 14 bits par
            // dimension (les 2 bits hauts sont l'échelle).
            if data.get(23..26)? != [0x9D, 0x01, 0x2A] {
                return None;
            }
            let width = le_u16(data, 26)? & 0x3FFF;
            let height = le_u16(data, 28)? & 0x3FFF;
            Some((u32::from(width), u32::from(height)))
        }
        b"VP8L" => {
            if *data.get(20)? != 0x2F {
                return None;
            }
            let bits = u32::from_le_bytes(data.get(21..25)?.try_into().ok()?);
            Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1))
        }
        b"VP8X" => Some((le_u24(data, 24)? + 1, le_u24(data, 27)? + 1)),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod samples {
    //! En-têtes d'exemple, construits octet par octet.

    pub fn png(width: u32, height: u32) -> Vec<u8> {
        let mut data = b"\x89PNG\r\n\x1a\n".to_vec();
        data.extend_from_slice(&13u32.to_be_bytes());
        data.extend_from_slice(b"IHDR");
        data.extend_from_slice(&width.to_be_bytes());
        data.extend_from_slice(&height.to_be_bytes());
        data.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        data.extend_from_slice(b"\0\0\0\0IEND\xAE\x42\x60\x82");
        data
    }

    pub fn jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut data = vec![0xFF, 0xD8];
        // APP0 (JFIF) de 16 octets, puis un octet de bourrage avant SOF2.
        data.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
        data.extend_from_slice(b"JFIF\0\x01\x01\x00\x00\x01\x00\x01\x00\x00");
        data.extend_from_slice(&[0xFF, 0xFF, 0xC2, 0x00, 0x11, 0x08]);
        data.extend_from_slice(&height.to_be_bytes());
        data.extend_from_slice(&width.to_be_bytes());
        data.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
        // Données compressées : un 0xFF y est suivi de 0x00. Puis EOI.
        data.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02, 0x12, 0xFF, 0x00, 0x34]);
        data.extend_from_slice(&[0xFF, 0xD9]);
        data
    }

    fn riff(chunk: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut data = b"RIFF".to_vec();
        data.extend_from_slice(&((payload.len() + 12) as u32).to_le_bytes());
        data.extend_from_slice(b"WEBP");
        data.extend_from_slice(chunk);
        data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        data.extend_from_slice(payload);
        data
    }

    pub fn webp_lossy(width: u16, height: u16) -> Vec<u8> {
        let mut payload = vec![0x10, 0x02, 0x00, 0x9D, 0x01, 0x2A];
        // Échelle 1 dans les bits hauts de la largeur : elle doit être ignorée.
        payload.extend_from_slice(&(width | 0x4000).to_le_bytes());
        payload.extend_from_slice(&height.to_le_bytes());
        riff(b"VP8 ", &payload)
    }

    pub fn webp_lossless(width: u32, height: u32) -> Vec<u8> {
        let bits = (width - 1) | (height - 1) << 14;
        let mut payload = vec![0x2F];
        payload.extend_from_slice(&bits.to_le_bytes());
        riff(b"VP8L", &payload)
    }

    pub fn webp_extended(width: u32, height: u32) -> Vec<u8> {
        let mut payload = vec![0x10, 0, 0, 0];
        payload.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
        payload.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
        riff(b"VP8X", &payload)
    }
}

#[cfg(test)]
mod tests {
    use super::samples::*;
    use super::*;

    fn probe(format: ImageFormat, width: u32, height: u32) -> Result<Probe, Refusal> {
        Ok(Probe {
            format,
            width,
            height,
        })
    }

    #[test]
    fn reads_each_format_from_its_header() {
        assert_eq!(
            inspect(&png(3840, 2160)),
            probe(ImageFormat::Png, 3840, 2160)
        );
        assert_eq!(
            inspect(&jpeg(1920, 1080)),
            probe(ImageFormat::Jpeg, 1920, 1080)
        );
        assert_eq!(
            inspect(&webp_lossy(1280, 720)),
            probe(ImageFormat::Webp, 1280, 720)
        );
        assert_eq!(
            inspect(&webp_lossless(800, 600)),
            probe(ImageFormat::Webp, 800, 600)
        );
        assert_eq!(
            inspect(&webp_extended(8192, 4096)),
            probe(ImageFormat::Webp, 8192, 4096)
        );
    }

    #[test]
    fn format_comes_from_content_not_extension() {
        let gif = b"GIF89a\x01\x00\x01\x00".to_vec();
        assert_eq!(inspect(&gif), Err(Refusal::Format));
        assert_eq!(inspect(b"texte renomme en .png"), Err(Refusal::Format));
        assert_eq!(inspect(&[]), Err(Refusal::Format));
        // RIFF sans WEBP (un fichier WAV, par exemple).
        assert_eq!(inspect(b"RIFF\0\0\0\0WAVEfmt "), Err(Refusal::Format));
    }

    #[test]
    fn inconsistent_headers_are_unreadable() {
        let mut no_ihdr = png(10, 10);
        no_ihdr[12..16].copy_from_slice(b"IDAT");
        assert_eq!(inspect(&no_ihdr), Err(Refusal::Unreadable));

        let truncated = &png(10, 10)[..18];
        assert_eq!(inspect(truncated), Err(Refusal::Unreadable));
        assert_eq!(inspect(&png(0, 10)), Err(Refusal::Unreadable));

        let mut bad_sync = webp_lossy(10, 10);
        bad_sync[23] = 0;
        assert_eq!(inspect(&bad_sync), Err(Refusal::Unreadable));

        let mut bad_signature = webp_lossless(10, 10);
        bad_signature[20] = 0;
        assert_eq!(inspect(&bad_signature), Err(Refusal::Unreadable));

        assert_eq!(inspect(&[0xFF, 0xD8, 0xFF]), Err(Refusal::Unreadable));
        // Début des données compressées (SOS) sans SOF : illisible.
        assert_eq!(
            inspect(&[0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x02]),
            Err(Refusal::Unreadable)
        );
        // Longueur de segment impossible.
        assert_eq!(
            inspect(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x01]),
            Err(Refusal::Unreadable)
        );
    }

    #[test]
    fn truncated_files_are_unreadable() {
        let png = png(640, 480);
        assert_eq!(inspect(&png[..png.len() - 12]), Err(Refusal::Unreadable));
        let mut with_trailer = png.clone();
        with_trailer.extend_from_slice(b"octets en trop");
        assert!(inspect(&with_trailer).is_ok());

        let jpeg = jpeg(640, 480);
        assert_eq!(inspect(&jpeg[..jpeg.len() - 2]), Err(Refusal::Unreadable));
        // Vignette EXIF terminée par FF D9 avant l'image principale : la
        // fin de l'image principale manque toujours.
        let mut thumbnail = vec![0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x06, 0xFF, 0xD9, 0x00, 0x00];
        thumbnail.extend_from_slice(&jpeg[2..jpeg.len() - 2]);
        assert_eq!(inspect(&thumbnail), Err(Refusal::Unreadable));

        let webp = webp_lossy(640, 480);
        assert_eq!(inspect(&webp[..webp.len() - 1]), Err(Refusal::Unreadable));
    }

    /// Certains téléphones ajoutent des données après le marqueur de fin
    /// (bloc final de plusieurs Kio, qui peut contenir des 0xFF) : le
    /// fichier reste valide tant que le marqueur est présent.
    #[test]
    fn jpeg_with_data_after_the_end_marker_is_accepted() {
        let mut with_trailer = jpeg(4000, 3000);
        with_trailer.extend_from_slice(b"SEFH");
        with_trailer.extend(std::iter::repeat_n([0xFF, 0x00, 0x12, 0xFF], 16 * 1024).flatten());
        with_trailer.extend_from_slice(b"SEFT");
        assert_eq!(
            inspect(&with_trailer),
            Ok(Probe {
                format: ImageFormat::Jpeg,
                width: 4000,
                height: 3000
            })
        );
    }

    #[test]
    fn dimensions_are_limited_by_side_and_surface() {
        assert!(inspect(&png(8192, 4882)).is_ok());
        assert_eq!(
            inspect(&png(8193, 100)),
            Err(Refusal::Dimensions {
                width: 8193,
                height: 100
            })
        );
        // 8 000 × 5 001 : chaque côté passe, pas la surface.
        assert_eq!(
            inspect(&png(8000, 5001)),
            Err(Refusal::Dimensions {
                width: 8000,
                height: 5001
            })
        );
        assert_eq!(
            inspect(&webp_extended(16_000, 16_000)),
            Err(Refusal::Dimensions {
                width: 16_000,
                height: 16_000
            })
        );
    }

    #[test]
    fn size_limit_is_10_mib() {
        assert_eq!(check_size(MAX_BYTES), Ok(()));
        assert_eq!(
            check_size(MAX_BYTES + 1),
            Err(Refusal::TooLarge {
                bytes: MAX_BYTES + 1
            })
        );
    }

    #[test]
    fn display_name_keeps_only_a_clean_file_name() {
        assert_eq!(
            display_name(Path::new("/home/moi/Images/nebuleuse-nord.jpg")),
            "nebuleuse-nord.jpg"
        );
        assert_eq!(display_name(Path::new("a\u{7}b\nc.png")), "abc.png");
        assert_eq!(display_name(Path::new("/")), "image");
        let long = "x".repeat(300);
        assert_eq!(display_name(Path::new(&long)).chars().count(), 128);
    }
}
