//! Connexion SSH unique (russh) partagée par toute l'application.
//!
//! Expose deux primitives sur cette connexion : exécuter une commande et
//! récupérer sa sortie, ouvrir un canal en streaming. Chaque opération ouvre
//! son propre canal.

pub mod host_key;
