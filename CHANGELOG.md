# Changelog

Changements notables d'Atlas, du plus récent au plus ancien.

Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) ; numéros de version selon le [versionnage sémantique](https://semver.org/lang/fr/) :

- **correctif** (1.0.**1**) : uniquement des corrections ;
- **mineur** (1.**1**.0) : nouvelles fonctions, rien d'existant ne change ;
- **majeur** (**2**.0.0) : changement incompatible pour l'utilisateur (profils ou scripts d'une ancienne version illisibles, nouvelle configuration du VPS exigée, fonction retirée).

Catégories : Ajouté, Modifié, Corrigé, Retiré, Sécurité.

## [Non publié]

### Corrigé

- Réglages : le texte des boutons du formulaire de profil avait la mauvaise couleur ; « Enregistrer » était invisible (ambre sur ambre).

## [1.0.0] - 2026-10-19

Première version publiée, installable sous Windows et Linux.

### Ajouté

- Connexion SSH par clé, une seule liaison active, avec vérification de l'empreinte du serveur (validation à la première connexion, blocage si elle change) et passphrase mémorisée dans le trousseau du système.
- Profils de connexion : plusieurs profils, un profil par défaut, ajout, modification et suppression.
- Tableau de bord : conteneurs Docker et services systemd utilisateur, démarrage, arrêt et redémarrage avec confirmation et vérification de l'état réel, logs en direct.
- Création d'un service systemd par formulaire, avec aperçu du fichier d'unité, et suppression d'un service.
- Explorateur de fichiers (SFTP) : navigation, création, renommage, suppression avec confirmation, envoi et téléchargement.
- Scripts personnalisés : enregistrés localement, confirmation facultative, exécution avec sortie en direct, dernière exécution mémorisée.
- Écran Télémétrie : onglets de logs et de sorties de scripts, qui continuent pendant la navigation.
- Installateurs : NSIS pour Windows (utilisateur courant, sans droits administrateur), .deb et AppImage pour Linux.

### Sécurité

- Politique de sécurité du contenu stricte et permissions de l'application réduites à ce qui est utilisé.
- Aucune commande avec `sudo` ; valeurs envoyées au serveur validées puis échappées ; opérations sur les fichiers uniquement par SFTP.

[Non publié]: https://github.com/Poutoo/vpsctl/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/Poutoo/vpsctl/releases/tag/v1.0.0
