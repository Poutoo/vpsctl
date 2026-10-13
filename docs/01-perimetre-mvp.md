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
- Exécution avec sortie affichée en direct

## Hors MVP

- Création de conteneurs Docker : remplacée en v2 par une gestion `docker compose` (up/down sur un dossier de projet)
- Édition de fichiers texte dans l'appli
- Terminal intégré
- Monitoring CPU, RAM, disque
- Interface multi-VPS (le modèle de données supporte déjà plusieurs profils, seule l'interface est limitée à un)
- Services lancés autrement que par Docker ou systemd (screen, tmux, pm2)