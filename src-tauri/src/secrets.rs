//! Surcouche de la crate `keyring` pour la passphrase de la clé SSH
//! (Credential Manager sous Windows, Secret Service sous Linux).
//!
//! La passphrase n'est jamais écrite dans un fichier, jamais journalisée et
//! jamais renvoyée au frontend. Les messages d'erreur ne la contiennent pas.
//! Les appels au trousseau sont synchrones : l'appelant les exécute hors du
//! fil asynchrone (`spawn_blocking`).
//!
//! Le service du trousseau est l'identifiant de l'application, lu au
//! démarrage : le build des tests E2E (`dev.poutoo.atlas.e2e`) n'atteint
//! jamais les passphrases de l'application installée.

use zeroize::Zeroizing;

use crate::error::AppError;

/// Trousseau de l'application : un service, un compte par profil.
#[derive(Debug, Clone)]
pub struct Keyring {
    service: String,
}

impl Keyring {
    /// `service` : identifiant de l'application (`identifier` de Tauri).
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, profile_id: &str) -> Result<keyring::Entry, AppError> {
        keyring::Entry::new(&self.service, &account(profile_id))
            .map_err(|e| AppError::io(format!("trousseau du système indisponible : {e}")))
    }

    /// Passphrase mémorisée pour ce profil, si elle existe.
    pub fn get_passphrase(&self, profile_id: &str) -> Result<Option<Zeroizing<String>>, AppError> {
        match self.entry(profile_id)?.get_password() {
            Ok(passphrase) => Ok(Some(Zeroizing::new(passphrase))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::io(format!(
                "lecture du trousseau impossible : {e}"
            ))),
        }
    }

    /// Mémorise (ou remplace) la passphrase de ce profil.
    pub fn set_passphrase(&self, profile_id: &str, passphrase: &str) -> Result<(), AppError> {
        self.entry(profile_id)?
            .set_password(passphrase)
            .map_err(|e| AppError::io(format!("écriture dans le trousseau impossible : {e}")))
    }

    /// Supprime la passphrase de ce profil ; absente, ce n'est pas une erreur.
    pub fn delete_passphrase(&self, profile_id: &str) -> Result<(), AppError> {
        match self.entry(profile_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::io(format!(
                "suppression dans le trousseau impossible : {e}"
            ))),
        }
    }
}

fn account(profile_id: &str) -> String {
    format!("passphrase:{profile_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identifier(conf: &str) -> String {
        let conf: serde_json::Value = serde_json::from_str(conf).unwrap();
        conf["identifier"].as_str().unwrap().to_owned()
    }

    /// Les passphrases déjà enregistrées par l'application installée sont
    /// sous ce service : le changer les rendrait inaccessibles.
    #[test]
    fn installed_application_keeps_its_keyring_service() {
        assert_eq!(
            identifier(include_str!("../tauri.conf.json")),
            "dev.poutoo.atlas"
        );
    }

    #[test]
    fn e2e_build_has_its_own_keyring_service() {
        let e2e = identifier(include_str!("../../e2e/tauri.e2e.conf.json"));
        assert_eq!(e2e, "dev.poutoo.atlas.e2e");
        assert_ne!(e2e, identifier(include_str!("../tauri.conf.json")));
    }

    #[test]
    fn one_account_per_profile() {
        assert_eq!(account("abc"), "passphrase:abc");
        assert_ne!(account("a"), account("b"));
    }
}
