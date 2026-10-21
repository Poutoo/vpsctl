# Changelog

Changements notables d'Atlas, du plus récent au plus ancien.

Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) ; numéros de version selon le [versionnage sémantique](https://semver.org/lang/fr/) :

- **correctif** (1.0.**1**) : uniquement des corrections ;
- **mineur** (1.**1**.0) : nouvelles fonctions, rien d'existant ne change ;
- **majeur** (**2**.0.0) : changement incompatible pour l'utilisateur (profils ou scripts d'une ancienne version illisibles, nouvelle configuration du VPS exigée, fonction retirée).

Catégories : Ajouté, Modifié, Corrigé, Retiré, Sécurité.

## [Non publié]

### Ajouté

- Perte de liaison : l'application reste sur l'écran affiché avec un bandeau LOS (durée, dernier échange, cause) et un bouton « Rétablir la liaison » ; aucune reconnexion automatique. Les tableaux et l'explorateur restent lisibles, figés sur leur dernière lecture.
- Filtre de recherche sur le tableau de bord (conteneurs et services) et dans l'explorateur de fichiers, sans tenir compte de la casse ni des accents, avec Ctrl+F pour y placer le curseur et Échap pour le vider ; il reste utilisable pendant une perte de liaison.
- Services systemd : colonne « Au démarrage » avec un interrupteur pour activer ou désactiver le démarrage automatique d'un service (sans le démarrer ni l'arrêter) ; les états non modifiables (statique, masqué, instance de gabarit…) sont affichés en lecture seule avec leur explication.
- Conteneurs Docker : un conteneur tué par manque de mémoire (code 137 et `OOMKilled`) est affiché en échec avec la mention « Mémoire » et la limite en infobulle ; un code 137 sans manque de mémoire reste « Arrêté », avec sa cause en infobulle.
- Après une perte de liaison : « Reprendre » relance un flux de logs à partir de cet instant, « Relancer » un script (signalé « Liaison perdue · issue inconnue »), « Réessayer » un transfert depuis le début et « Reprendre la file » les transferts en attente.

### Modifié

- Une liaison coupée sans fermeture (réseau perdu, mise en veille) est détectée en 15 à 20 s, au lieu de 2 minutes environ ; une coupure de quelques secondes reste sans effet.

### Corrigé

- Tableau de bord : une coupure réseau de quelques secondes n'affiche plus les sections Docker et systemd en échec au bout de 10 s ; le rafraîchissement attend le retour de la liaison.
- Télémétrie : l'onglet d'un script coupé par une perte de liaison indique « Issue inconnue » au lieu d'« Interrompu ».
- Transferts : après une erreur réseau, la file est suspendue (« Reprendre la file ») au lieu de lancer les transferts suivants, qui échouaient l'un après l'autre ; un transfert coupé par une perte de liaison avant d'avoir commencé revient en attente.
- Transferts : un envoi retenu par une perte de liaison ne remplace plus un fichier existant sans redemander la confirmation à la reprise de la file.
- Réglages : le texte des boutons du formulaire de profil avait la mauvaise couleur ; « Enregistrer » était invisible (ambre sur ambre).
- Scripts : un nom contenant des émojis était refusé par le formulaire avant 64 caractères, alors que l'application l'acceptait.
- Formulaires (service, script, profil) : un caractère invisible en début ou en fin de champ (U+0085, U+FEFF) pouvait donner un verdict différent de celui de l'enregistrement.

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
