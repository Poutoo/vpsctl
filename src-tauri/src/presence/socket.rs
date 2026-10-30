//! Connexion locale à Discord : tube nommé sous Windows, socket Unix sous
//! Linux. Aucune connexion réseau.
//!
//! Emplacements sous Linux (documentation RPC de Discord pour les dossiers
//! de base ; sous-dossiers Flatpak et Snap relevés dans les crates
//! `discord-rich-presence` et `discord-presence`, qui concordent) :
//! - `$XDG_RUNTIME_DIR`, `$TMPDIR`, `$TMP`, `$TEMP`, puis `/tmp` ;
//! - dans chacun : `discord-ipc-N`, `app/com.discordapp.Discord/discord-ipc-N`
//!   (Flatpak) et `snap.discord/discord-ipc-N` (Snap), N de 0 à 9.

use std::path::PathBuf;

/// Sous-dossiers essayés dans chaque dossier de base.
#[cfg_attr(not(unix), allow(dead_code))]
const SUBDIRS: [&str; 3] = ["", "app/com.discordapp.Discord", "snap.discord"];

#[cfg_attr(not(unix), allow(dead_code))]
pub fn unix_candidates(env: impl Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let mut bases: Vec<PathBuf> = Vec::new();
    let from_env = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .into_iter()
        .filter_map(&env)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    for base in from_env.chain([PathBuf::from("/tmp")]) {
        if !bases.contains(&base) {
            bases.push(base);
        }
    }
    let mut paths = Vec::new();
    for base in &bases {
        for n in 0..10 {
            for sub in SUBDIRS {
                paths.push(base.join(sub).join(format!("discord-ipc-{n}")));
            }
        }
    }
    paths
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn windows_candidates() -> Vec<PathBuf> {
    (0..10)
        .map(|n| PathBuf::from(format!(r"\\?\pipe\discord-ipc-{n}")))
        .collect()
}

#[cfg(windows)]
pub type Stream = tokio::net::windows::named_pipe::NamedPipeClient;
#[cfg(unix)]
pub type Stream = tokio::net::UnixStream;

/// Premier emplacement qui répond.
pub async fn connect() -> std::io::Result<Stream> {
    #[cfg(windows)]
    {
        for path in windows_candidates() {
            if let Ok(pipe) = tokio::net::windows::named_pipe::ClientOptions::new().open(&path) {
                return Ok(pipe);
            }
        }
    }
    #[cfg(unix)]
    {
        for path in unix_candidates(|key| std::env::var(key).ok()) {
            if let Ok(socket) = tokio::net::UnixStream::connect(&path).await {
                return Ok(socket);
            }
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Discord ne répond pas",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn linux_paths_cover_native_flatpak_and_snap() {
        let env: HashMap<&str, &str> =
            [("XDG_RUNTIME_DIR", "/run/user/1000"), ("TMPDIR", "")].into();
        let paths = unix_candidates(|key| env.get(key).map(|v| v.to_string()));
        let first: Vec<_> = paths
            .iter()
            .take(4)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(
            first,
            [
                "/run/user/1000/discord-ipc-0",
                "/run/user/1000/app/com.discordapp.Discord/discord-ipc-0",
                "/run/user/1000/snap.discord/discord-ipc-0",
                "/run/user/1000/discord-ipc-1",
            ]
        );
        // Dossier vide ignoré, /tmp en dernier recours : 2 bases × 10 × 3.
        assert_eq!(paths.len(), 60);
        assert!(paths.last().unwrap().starts_with("/tmp"));
    }

    #[test]
    fn duplicate_bases_are_tried_once() {
        let env: HashMap<&str, &str> = [("TMPDIR", "/tmp"), ("TMP", "/tmp")].into();
        assert_eq!(
            unix_candidates(|key| env.get(key).map(|v| v.to_string())).len(),
            30
        );
    }

    #[test]
    fn windows_pipes_are_numbered_0_to_9() {
        let pipes = windows_candidates();
        assert_eq!(pipes.len(), 10);
        assert_eq!(pipes[0], PathBuf::from(r"\\?\pipe\discord-ipc-0"));
    }
}
