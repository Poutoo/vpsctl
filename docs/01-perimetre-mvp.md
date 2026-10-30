# Périmètre du MVP

Statut : validé.

## Explorateur de fichiers (SFTP)

- Naviguer dans l'arborescence
- Créer un fichier, créer un dossier
- Renommer
- Supprimer (avec confirmation)
- Envoyer un fichier vers le VPS
- Télécharger un fichier depuis le VPS

L'édition du contenu des fichiers n'est pas dans le MVP.

## Docker

- Lister tous les conteneurs, y compris arrêtés
- Démarrer, arrêter, redémarrer
- Afficher les logs en direct
- Supprimer un conteneur (avec confirmation). Uniquement le conteneur : ni ses images, ni ses volumes.

## Services systemd (utilisateur)

Il s'agit de services utilisateur (`systemctl --user`), pas de services système.

- Lister tous les services utilisateur (pas une sélection manuelle, pour que les services créés depuis l'appli apparaissent)
- Voir le statut
- Démarrer, arrêter, redémarrer
- Afficher les logs en direct (`journalctl --user -f`)
- Créer un service via un formulaire qui génère le fichier d'unité
- Supprimer un service (stop, disable, suppression du fichier, `daemon-reload`)

## Scripts personnalisés

- Stockés localement dans un fichier JSON de l'application
- Chaque script : nom, commande, confirmation obligatoire ou non
- Description facultative
- Dernière exécution mémorisée localement (date et code de sortie)
- Ajout, modification et suppression depuis l'appli
- Exécution avec sortie affichée en direct

## Profils de connexion

- Plusieurs profils, **une seule connexion active à la fois**
- Un profil peut être marqué « par défaut » (présélectionné à l'ouverture)
- Ajout, modification et suppression (avec confirmation)

## Hors MVP

- Création de conteneurs Docker : remplacée en v1.3 par une gestion `docker compose` (up/down sur un dossier de projet, jalon 11)
- Édition de fichiers texte dans l'appli : v1.3 (jalon 12)
- Terminal intégré
- Monitoring CPU, RAM, disque
- Connexions simultanées à plusieurs VPS
- Oublier l'empreinte ou la passphrase depuis les réglages (le remplacement d'empreinte passe par l'alerte de sécurité ; une passphrase erronée est simplement redemandée puis remplacée dans le trousseau)
- Modification des droits des fichiers
- Services lancés autrement que par Docker ou systemd (screen, tmux, pm2)
- Activer ou désactiver un service au démarrage (`systemctl --user enable/disable`) : v1.1 (jalon 9)
- Filtre de recherche dans les listes : v1.1 (jalon 9)
