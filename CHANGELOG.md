# Changelog

Changements notables d'Atlas, du plus récent au plus ancien.

Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) ; numéros de version selon le [versionnage sémantique](https://semver.org/lang/fr/) :

- **correctif** (1.0.**1**) : uniquement des corrections ;
- **mineur** (1.**1**.0) : nouvelles fonctions, rien d'existant ne change ;
- **majeur** (**2**.0.0) : changement incompatible pour l'utilisateur (profils ou scripts d'une ancienne version illisibles, nouvelle configuration du VPS exigée, fonction retirée).

Catégories : Ajouté, Modifié, Corrigé, Retiré, Sécurité.

## [Non publié]

### Ajouté

- Barre d'état : version du plugin docker compose du serveur, ou « Compose absent ».
- Tableau de bord : les conteneurs Docker sont groupés par projet docker compose (dossier, fichiers de configuration, état d'ensemble Nominal, Partiel, Arrêté, En échec ou En transition, avec le nombre de conteneurs en marche), puis « Hors compose » pour les autres. Un conteneur tué par manque de mémoire ou en boucle de redémarrage met son projet en échec. Les groupes se replient.
- Projets compose mémorisés par profil : un projet qui n'a plus aucun conteneur reste affiché « Descendu », jusqu'à « Oublier ce projet » (sans rien toucher sur le serveur). Si le plugin compose est absent, les projets restent affichés et un avis le signale.
- Projets compose : le dossier et les fichiers de configuration et d'environnement sont vérifiés toutes les 30 secondes ; un dossier supprimé, un fichier manquant ou illisible pour le compte connecté est signalé sur le projet.
- Actions de projet compose : Démarrer (`up -d`), Récupérer les images (`pull`), Redémarrer et Arrêter et supprimer (`down`, avec un GO / NO GO qui liste les conteneurs et le réseau supprimés, les volumes nommés, volumes anonymes et dossiers montés conservés). Aucune action ne supprime de volume ni d'image. Démarrer et Récupérer sont indisponibles, avec leur raison, si le dossier ou un fichier manque ou est illisible ; Redémarrer et Arrêter restent possibles.
- Chaque commande compose s'affiche dans son onglet de l'écran Télémétrie (6 au plus), avec la commande exacte et un chrono pendant son exécution ; elle continue si l'on change d'écran, une seule à la fois par projet, signalée dans la navigation. Seul son code de sortie décide du succès ; un échec reste indiqué sur le projet jusqu'à la commande suivante. L'interrompre demande une confirmation ; une perte de liaison laisse son issue inconnue, sans relance automatique.
- Éditeur de fichiers texte dans l'écran Fichiers : « Modifier » dans le menu d'un fichier, double-clic ou Entrée ; l'itinéraire garde le chemin. Fichiers UTF-8 jusqu'à 1 Mo, numéros de ligne et lignes modifiées marquées, sans coloration syntaxique. Tab insère une tabulation ; Échap puis Tab sort de la zone de texte.
- Enregistrement (Ctrl+S ou Enregistrer) qui conserve le BOM, les fins de ligne (LF ou CRLF) et les droits du fichier : le texte est écrit dans un fichier temporaire du même dossier, qui remplace l'original d'un seul coup, jamais à moitié écrit. « Identique au serveur » s'affiche après une relecture qui le vérifie.
- Si le fichier a changé sur le serveur depuis son ouverture, rien n'est écrit : la fenêtre de conflit dit ce que chaque choix efface et propose Annuler, Recharger la version du serveur ou Écraser.
- Fichiers refusés par l'éditeur, avec la raison : plus de 1 Mo, contenu binaire (position du premier octet nul), UTF-8 invalide (position de la première erreur) et lien symbolique ; le téléchargement reste proposé, sauf pour une clé privée, jamais affichée ni téléchargée depuis l'éditeur, où qu'elle se trouve.
- Lecture seule, avec sa raison : fichier non inscriptible, appartenant à un autre compte, dans un dossier non inscriptible, ou aux fins de ligne mélangées. Le contenu reste lisible, copiable et téléchargeable.
- Fermer l'éditeur, changer d'écran, couper la liaison ou fermer Atlas avec des modifications non enregistrées demande confirmation. Pendant une perte de liaison, le texte est conservé et l'enregistrement attend le rétablissement, puis vérifie que le fichier n'a pas changé.
- Vue Différence depuis la fenêtre de conflit, en lecture seule : version du serveur et la vôtre ligne à ligne, chaque changement attribué au serveur (ambre) ou à vous, avec une action par bloc (Prendre la version du serveur, Reprendre, Retirer). Là où vos changements et ceux du serveur se mêlent, l'action annonce les lignes à vous qu'elle efface ; Ctrl+Z l'annule dans l'édition. « Enregistrer… » écrase en disant ce qui serait encore perdu.
- Fichiers de `~/.ssh` dans l'éditeur : bandeau permanent. Pour `authorized_keys`, chaque enregistrement passe par un GO / NO GO qui vérifie que la clé de la liaison en cours reste autorisée (avertissement fort si elle est retirée, constat neutre si elle était déjà absente), compte les clés reconnues (options et certificats compris ; une ligne illisible est signalée « non reconnue par Atlas »), liste les clés ajoutées et retirées et rappelle que les droits sont conservés. La case « autre moyen d'accès » est requise et se décoche à chaque enregistrement.

## [1.2.0] - 2026-10-30

### Ajouté

- Réglages : onglets Profils et Préférences. Les préférences règlent l'assombrissement de l'image de fond (85 % par défaut), avec un aperçu en direct de la lisibilité, et proposent d'afficher l'image dans ses couleurs d'origine. Elles sont conservées d'un lancement à l'autre ; un fichier de préférences absent ou abîmé redonne les réglages par défaut sans gêner le démarrage.
- Fond d'écran personnalisé : une image PNG, JPEG ou WebP (10 Mo et 8 192 px de côté au plus) choisie sur l'ordinateur est copiée dans le dossier de configuration d'Atlas et s'affiche dans toute l'application, écran de connexion compris. Un fichier d'un autre format, trop lourd, trop grand ou abîmé est refusé avec sa raison, sans changer le fond. « Rétablir le fond d'origine » supprime la copie. Si la copie a disparu ou est devenue illisible, Atlas reprend le fond d'origine et le signale au lancement.
- Discord Rich Presence, désactivée par défaut : une fois activée dans les préférences, la carte « Atlas · VPS Control » affiche sur votre profil Discord « En liaison » ou « Au sol », la durée écoulée depuis le lancement d'Atlas (elle continue quand l'état change) et un bouton « Source Code » vers le dépôt GitHub d'Atlas, rien d'autre (ni profil, ni serveur, ni fichier, ni commande). Discord fermé ou absent : Atlas réessaie sans bruit et propose « Réessayer ». La carte est effacée quand on désactive l'interrupteur et à la fermeture d'Atlas. Disponible uniquement dans l'application installée.

## [1.1.0] - 2026-10-21

### Ajouté

- Perte de liaison : l'application reste sur l'écran affiché avec un bandeau LOS (durée, dernier échange, cause) et un bouton « Rétablir la liaison » ; aucune reconnexion automatique. Les tableaux et l'explorateur restent lisibles, figés sur leur dernière lecture.
- Après une perte de liaison : « Reprendre » relance un flux de logs à partir de cet instant, « Relancer » un script (signalé « Liaison perdue · issue inconnue »), « Réessayer » un transfert depuis le début et « Reprendre la file » les transferts en attente.
- Filtre de recherche sur le tableau de bord (conteneurs et services) et dans l'explorateur de fichiers, sans tenir compte de la casse ni des accents, avec Ctrl+F pour y placer le curseur et Échap pour le vider ; il reste utilisable pendant une perte de liaison.
- Services systemd : colonne « Au démarrage » avec un interrupteur pour activer ou désactiver le démarrage automatique d'un service (sans le démarrer ni l'arrêter) ; les états non modifiables (statique, masqué, instance de gabarit…) sont affichés en lecture seule avec leur explication.
- Conteneurs Docker : un conteneur tué par manque de mémoire (code 137 et `OOMKilled`) est affiché en échec avec la mention « Mémoire » et la limite en infobulle ; un code 137 sans manque de mémoire reste « Arrêté », avec sa cause en infobulle.

### Modifié

- Barre de titre : le symbole d'Atlas (papillon) précède son nom ; en fenêtre étroite, « Contrôle VPS » est masqué pour laisser la place à l'indicateur de liaison.
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

[Non publié]: https://github.com/Poutoo/vpsctl/compare/v1.2.0...HEAD
[1.2.0]: https://github.com/Poutoo/vpsctl/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/Poutoo/vpsctl/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/Poutoo/vpsctl/releases/tag/v1.0.0
