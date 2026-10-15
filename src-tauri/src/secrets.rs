//! Surcouche de la crate `keyring` pour la passphrase de la clé SSH
//! (Credential Manager sous Windows, Secret Service sous Linux).
//!
//! La passphrase n'est jamais écrite dans un fichier, jamais journalisée et
//! jamais renvoyée au frontend. Les messages d'erreur ne la contiennent pas.
//! Les appels au trousseau sont synchrones : l'appelant les exécute hors du
//! fil asynchrone (`spawn_blocking`).

use zeroize::Zeroizing;

use crate::error::AppError;

/// Service du trousseau : identique à l'`identifier` de `tauri.conf.json`.
pub const SERVICE: &str = "dev.poutoo.atlas";

fn account(profile_id: &str) -> String {
    format!("passphrase:{profile_id}")
}

fn entry(profile_id: &str) -> Result<keyring::Entry, AppError> {
    keyring::Entry::new(SERVICE, &account(profile_id))
        .map_err(|e| AppError::io(format!("trousseau du système indisponible : {e}")))
}

/// Passphrase mémorisée pour ce profil, si elle existe.
pub fn get_passphrase(profile_id: &str) -> Result<Option<Zeroizing<String>>, AppError> {
    match entry(profile_id)?.get_password() {
        Ok(passphrase) => Ok(Some(Zeroizing::new(passphrase))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::io(format!(
            "lecture du trousseau impossible : {e}"
        ))),
    }
}

/// Mémorise (ou remplace) la passphrase de ce profil.
pub fn set_passphrase(profile_id: &str, passphrase: &str) -> Result<(), AppError> {
    entry(profile_id)?
        .set_password(passphrase)
        .map_err(|e| AppError::io(format!("écriture dans le trousseau impossible : {e}")))
}

/// Supprime la passphrase de ce profil ; absente, ce n'est pas une erreur.
pub fn delete_passphrase(profile_id: &str) -> Result<(), AppError> {
    match entry(profile_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::io(format!(
            "suppression dans le trousseau impossible : {e}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_matches_the_tauri_identifier() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], SERVICE);
    }

    #[test]
    fn one_account_per_profile() {
        assert_eq!(account("abc"), "passphrase:abc");
        assert_ne!(account("a"), account("b"));
    }
}
