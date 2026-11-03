//! Fonctions pures sur le contenu d'un fichier édité : refus (clé privée,
//! binaire, UTF-8 invalide), BOM et fins de ligne à l'ouverture, encodage
//! fidèle à l'enregistrement, empreinte SHA-256.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::error::AppError;

/// Taille maximale d'un fichier ouvert dans l'éditeur (1 Mio).
pub const MAX_SIZE: u64 = 1_048_576;
/// Octets examinés pour reconnaître un fichier binaire.
pub const BINARY_PROBE: usize = 8_192;
const BOM: &[u8] = b"\xEF\xBB\xBF";

/// Style de fin de ligne détecté à l'ouverture. `Mixed` : CRLF et LF dans le
/// même fichier, ou un CR seul ; impossible à conserver fidèlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LineEnding {
    Lf,
    Crlf,
    Mixed,
}

/// Raison du refus d'ouvrir un fichier. Le contenu d'un fichier refusé n'est
/// jamais envoyé à l'interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "reason",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, rename = "OpenRefusal")]
pub enum Refusal {
    TooLarge {
        #[ts(type = "number")]
        size: u64,
        #[ts(type = "number")]
        limit: u64,
    },
    /// Premier octet nul parmi les `BINARY_PROBE` premiers.
    Binary {
        #[ts(type = "number")]
        offset: u64,
    },
    /// Première séquence UTF-8 invalide : position en octets et ligne.
    InvalidUtf8 {
        #[ts(type = "number")]
        offset: u64,
        line: u32,
    },
    /// En-tête `-----BEGIN … PRIVATE KEY-----`, sur n'importe quelle ligne.
    PrivateKey { line: u32, header: String },
    /// L'explorateur ne suit jamais les liens : l'éditeur non plus.
    Symlink { target: Option<String> },
    /// Dossier, socket, périphérique…
    NotAFile,
}

impl Refusal {
    /// Le refus propose de télécharger le fichier. Jamais pour une clé
    /// privée, qui ne doit pas transiter par l'application.
    pub fn downloadable(&self) -> bool {
        matches!(
            self,
            Self::TooLarge { .. } | Self::Binary { .. } | Self::InvalidUtf8 { .. }
        )
    }
}

/// Contenu d'un fichier texte prêt pour l'éditeur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// Texte avec des fins de ligne LF (une zone de texte HTML convertit
    /// les CRLF en LF de toute façon).
    pub text: String,
    pub bom: bool,
    pub line_ending: LineEnding,
}

/// Empreinte SHA-256, en hexadécimal minuscule.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Lit le contenu d'un fichier, ou donne la raison du refus. Ordre : clé
/// privée (le contenu ne doit jamais être affiché), binaire, UTF-8 invalide
/// sur tout le fichier.
pub fn decode(bytes: &[u8]) -> Result<Decoded, Refusal> {
    if let Some((line, header)) = private_key_header(bytes) {
        return Err(Refusal::PrivateKey { line, header });
    }
    if let Some(offset) = bytes.iter().take(BINARY_PROBE).position(|&b| b == 0) {
        return Err(Refusal::Binary {
            offset: offset as u64,
        });
    }
    let bom = bytes.starts_with(BOM);
    let body = if bom { &bytes[BOM.len()..] } else { bytes };
    let raw = match std::str::from_utf8(body) {
        Ok(raw) => raw,
        Err(error) => {
            let offset = error.valid_up_to() + if bom { BOM.len() } else { 0 };
            return Err(Refusal::InvalidUtf8 {
                offset: offset as u64,
                line: line_at(bytes, offset),
            });
        }
    };
    let line_ending = detect_line_ending(raw);
    let text = if raw.contains('\r') {
        raw.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        raw.to_owned()
    };
    Ok(Decoded {
        text,
        bom,
        line_ending,
    })
}

/// Numéro (à partir de 1) de la ligne qui contient l'octet `offset`.
fn line_at(bytes: &[u8], offset: usize) -> u32 {
    let newlines = bytes[..offset.min(bytes.len())]
        .iter()
        .filter(|&&b| b == b'\n')
        .count();
    u32::try_from(newlines + 1).unwrap_or(u32::MAX)
}

/// Première ligne de la forme `-----BEGIN <MOTS> PRIVATE KEY-----` (espaces
/// de bord et BOM ignorés) : `RSA`, `EC`, `OPENSSH`, `ENCRYPTED`, aucun…
fn private_key_header(bytes: &[u8]) -> Option<(u32, String)> {
    let body = bytes.strip_prefix(BOM).unwrap_or(bytes);
    body.split(|&b| b == b'\n')
        .enumerate()
        .find_map(|(index, line)| {
            let line = line.trim_ascii();
            let middle = line
                .strip_prefix(b"-----BEGIN ")?
                .strip_suffix(b"PRIVATE KEY-----")?;
            let words_ok = middle
                .iter()
                .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b' ');
            (words_ok && (middle.is_empty() || middle.ends_with(b" "))).then(|| {
                (
                    u32::try_from(index + 1).unwrap_or(u32::MAX),
                    String::from_utf8_lossy(line).into_owned(),
                )
            })
        })
}

fn detect_line_ending(text: &str) -> LineEnding {
    let bytes = text.as_bytes();
    let (mut crlf, mut lf, mut cr) = (0usize, 0usize, 0usize);
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                crlf += 1;
                i += 1;
            }
            b'\r' => cr += 1,
            b'\n' => lf += 1,
            _ => {}
        }
        i += 1;
    }
    match (crlf, lf, cr) {
        (_, _, 1..) => LineEnding::Mixed,
        (1.., 1.., _) => LineEnding::Mixed,
        (1.., 0, 0) => LineEnding::Crlf,
        _ => LineEnding::Lf,
    }
}

/// Octets à écrire : fins de ligne rétablies, BOM remis s'il existait.
/// Refuse un texte qui contient un CR ou un caractère nul (le texte de
/// l'éditeur n'a que des LF), un style mélangé et un résultat de plus de
/// `MAX_SIZE` octets.
pub fn encode(text: &str, bom: bool, line_ending: LineEnding) -> Result<Vec<u8>, AppError> {
    if line_ending == LineEnding::Mixed {
        return Err(AppError::invalid_input(
            "fins de ligne mélangées : le fichier est en lecture seule",
        ));
    }
    if text.contains('\r') || text.contains('\0') {
        return Err(AppError::invalid_input(
            "le texte contient un retour chariot ou un caractère nul",
        ));
    }
    let mut bytes = Vec::with_capacity(text.len() + 3);
    if bom {
        bytes.extend_from_slice(BOM);
    }
    match line_ending {
        LineEnding::Crlf => bytes.extend_from_slice(text.replace('\n', "\r\n").as_bytes()),
        _ => bytes.extend_from_slice(text.as_bytes()),
    }
    if bytes.len() as u64 > MAX_SIZE {
        return Err(AppError::invalid_input(format!(
            "le fichier dépasserait la limite de l'éditeur ({} octets au plus)",
            MAX_SIZE
        )));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_opens_with_lf() {
        let decoded = decode(b"PORT=8080\nHOST=db\n").unwrap();
        assert_eq!(decoded.text, "PORT=8080\nHOST=db\n");
        assert!(!decoded.bom);
        assert_eq!(decoded.line_ending, LineEnding::Lf);
    }

    #[test]
    fn empty_file_and_file_without_newline_are_lf() {
        assert_eq!(decode(b"").unwrap().line_ending, LineEnding::Lf);
        assert_eq!(decode(b"one line").unwrap().line_ending, LineEnding::Lf);
    }

    #[test]
    fn crlf_is_detected_and_restored() {
        let original = b"a=1\r\nb=2\r\n";
        let decoded = decode(original).unwrap();
        assert_eq!(decoded.text, "a=1\nb=2\n");
        assert_eq!(decoded.line_ending, LineEnding::Crlf);
        assert_eq!(
            encode(&decoded.text, decoded.bom, decoded.line_ending).unwrap(),
            original
        );
    }

    #[test]
    fn bom_is_kept() {
        let original = b"\xEF\xBB\xBFnom=\xC3\xA9t\xC3\xA9\n";
        let decoded = decode(original).unwrap();
        assert!(decoded.bom);
        assert_eq!(decoded.text, "nom=été\n");
        assert_eq!(
            encode(&decoded.text, true, LineEnding::Lf).unwrap(),
            original
        );
    }

    #[test]
    fn bom_and_crlf_together_round_trip() {
        let original = b"\xEF\xBB\xBFa\r\nb";
        let decoded = decode(original).unwrap();
        assert_eq!(
            encode(&decoded.text, decoded.bom, decoded.line_ending).unwrap(),
            original
        );
    }

    #[test]
    fn mixed_line_endings_are_detected() {
        assert_eq!(decode(b"a\r\nb\n").unwrap().line_ending, LineEnding::Mixed);
        assert_eq!(decode(b"a\rb").unwrap().line_ending, LineEnding::Mixed);
        assert_eq!(decode(b"a\r\nb\r").unwrap().line_ending, LineEnding::Mixed);
        assert_eq!(decode(b"a\r\nb\rc\n").unwrap().text, "a\nb\nc\n");
    }

    #[test]
    fn mixed_line_endings_cannot_be_saved() {
        assert!(encode("a\n", false, LineEnding::Mixed).is_err());
    }

    #[test]
    fn carriage_returns_and_nul_are_refused_at_save() {
        assert!(encode("a\r\nb", false, LineEnding::Lf).is_err());
        assert!(encode("a\0b", false, LineEnding::Lf).is_err());
    }

    #[test]
    fn save_is_limited_to_one_mebibyte() {
        let text = "a".repeat(MAX_SIZE as usize);
        assert!(encode(&text, false, LineEnding::Lf).is_ok());
        assert!(encode(&text, true, LineEnding::Lf).is_err());
        let lines = "a\n".repeat(MAX_SIZE as usize / 2);
        assert!(encode(&lines, false, LineEnding::Crlf).is_err());
    }

    #[test]
    fn nul_byte_in_the_first_8_kib_is_binary() {
        let mut bytes = b"SQLite format 3".to_vec();
        bytes.push(0);
        assert_eq!(decode(&bytes), Err(Refusal::Binary { offset: 15 }));
    }

    #[test]
    fn nul_byte_after_8_kib_is_invalid_utf8_rather_than_binary() {
        let mut bytes = vec![b'a'; BINARY_PROBE];
        bytes.push(0);
        // Le caractère nul est de l'UTF-8 valide : seul le binaire le refuse,
        // et seulement au début. Le fichier s'ouvre.
        assert!(decode(&bytes).is_ok());
    }

    #[test]
    fn invalid_utf8_is_found_beyond_the_probe() {
        let mut bytes = b"a\n".repeat(5_000);
        bytes.extend_from_slice(b"\xC3\x28 suite\n");
        assert_eq!(
            decode(&bytes),
            Err(Refusal::InvalidUtf8 {
                offset: 10_000,
                line: 5_001
            })
        );
    }

    #[test]
    fn invalid_utf8_offset_counts_the_bom() {
        assert_eq!(
            decode(b"\xEF\xBB\xBFab\xFF"),
            Err(Refusal::InvalidUtf8 { offset: 5, line: 1 })
        );
    }

    #[test]
    fn private_keys_are_refused_on_any_line() {
        for (content, line, header) in [
            (
                &b"-----BEGIN OPENSSH PRIVATE KEY-----\nb3Blbn\n"[..],
                1,
                "-----BEGIN OPENSSH PRIVATE KEY-----",
            ),
            (
                b"-----BEGIN CERTIFICATE-----\nMII\n-----END CERTIFICATE-----\n  -----BEGIN RSA PRIVATE KEY-----\r\n",
                4,
                "-----BEGIN RSA PRIVATE KEY-----",
            ),
            (
                b"\xEF\xBB\xBF-----BEGIN PRIVATE KEY-----\n",
                1,
                "-----BEGIN PRIVATE KEY-----",
            ),
            (
                b"KEY=x\n-----BEGIN ENCRYPTED PRIVATE KEY-----\n",
                2,
                "-----BEGIN ENCRYPTED PRIVATE KEY-----",
            ),
        ] {
            assert_eq!(
                decode(content),
                Err(Refusal::PrivateKey {
                    line,
                    header: header.to_owned()
                })
            );
        }
    }

    #[test]
    fn private_key_check_comes_before_binary() {
        let mut bytes = b"-----BEGIN EC PRIVATE KEY-----\n".to_vec();
        bytes.push(0);
        assert!(matches!(decode(&bytes), Err(Refusal::PrivateKey { .. })));
    }

    #[test]
    fn other_pem_blocks_and_mentions_are_not_private_keys() {
        for content in [
            &b"-----BEGIN PUBLIC KEY-----\n"[..],
            b"# -----BEGIN OPENSSH PRIVATE KEY----- dans un commentaire\n",
            b"-----BEGIN openssh PRIVATE KEY-----\n",
            b"-----BEGIN XPRIVATE KEY-----\n",
        ] {
            assert!(
                decode(content).is_ok(),
                "{}",
                String::from_utf8_lossy(content)
            );
        }
    }

    #[test]
    fn download_is_never_offered_for_a_private_key() {
        assert!(!Refusal::PrivateKey {
            line: 1,
            header: String::new()
        }
        .downloadable());
        assert!(Refusal::Binary { offset: 0 }.downloadable());
        assert!(!Refusal::Symlink { target: None }.downloadable());
    }

    #[test]
    fn sha256_matches_a_known_value() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
