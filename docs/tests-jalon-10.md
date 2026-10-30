# Tests manuels du jalon 10 : préférences, fond d'écran, Discord Rich Presence

À dérouler sur Windows puis sur Linux, avec un build local (`npm run tauri build`) installé, sauf mention contraire. Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure.

Dossier de configuration : `%APPDATA%\dev.poutoo.atlas\` sous Windows, `~/.config/dev.poutoo.atlas/` sous Linux.

## 0. Préparation

- [ ] Images de test dans un dossier quelconque :
  - `fond.png`, `fond.jpg`, `fond.webp` : trois images valides, de préférence en couleurs, de moins de 10 Mo ;
  - `lourd.png` : une image valide de plus de 10 Mo ;
  - `capture.gif` : un GIF, et `gif-renomme.png` : le même fichier renommé en `.png` ;
  - `texte.png` : un fichier texte renommé en `.png` ;
  - `coupee.jpg` : la première moitié d'un JPEG (`head -c 100000 fond.jpg > coupee.jpg`) ;
  - `abimee.png` : un PNG valide dont le milieu est écrasé (`cp fond.png abimee.png && printf 'xxxxxxxxxxxxxxxx' | dd of=abimee.png bs=1 seek=2000 conv=notrunc`) ;
  - `geante.png` : une image de plus de 8 192 px de côté ou de plus de 40 mégapixels (par exemple 9 000 × 5 000, très compressible : aplat de couleur), de moins de 10 Mo.
- [ ] Application Discord créée sur le portail (voir `docs/discord/README.md`) et son identifiant dans `CLIENT_ID` (`src-tauri/src/presence/mod.rs`) avant le build. Sans identifiant, seule la partie 6 (« Indisponible ») est vérifiable.

## 1. Tests automatisés

### Windows

- [ ] `npm ci` puis `npm test` : tous les tests passent (dont `preferences`, `presence`, `backgroundText`, `presenceText`).
- [ ] `cd src-tauri && cargo test` : tous les tests passent (dont `background`, `preferences`, `presence`, et `link_phase_follows_aos_los_and_disconnection` dans `tests/link_loss.rs`).
- [ ] `npm run lint`, `npm run typecheck`, `npx prettier --check .`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` : rien à signaler.
- [ ] `npm run tauri build` produit `Atlas_1.2.0_x64-setup.exe` ; l'application installée affiche la version 1.2.0 dans la barre d'état.

### Linux

- [ ] Mêmes commandes, mêmes résultats ; le build produit le .deb et l'AppImage en 1.2.0.

## 2. Préférences

- [ ] Réglages : le sélecteur **Profils / Préférences** est en haut à droite. Profils montre l'écran habituel (liste, formulaire, « Nouveau profil ») ; Préférences porte le surtitre « IV · PRÉFÉRENCES ».
- [ ] Sans liaison (depuis l'écran de connexion, « Nouveau profil » puis « Préférences ») : l'onglet est accessible, avec « Retour à la connexion ».
- [ ] Premier lancement (supprimer `preferences.json`) : assombrissement 85 %, couleurs d'origine décochées, fond d'origine, Discord désactivé.
- [ ] Remplacer le contenu de `preferences.json` par `pas du json`, relancer : l'application démarre normalement avec les valeurs par défaut ; un premier changement réécrit un fichier valide.
- [ ] `{"dimming": 150, "originalColors": true}` : assombrissement revenu à 85 %, couleurs d'origine cochées (chaque champ est lu à part).

## 3. Fond d'écran

### Formats acceptés

- [ ] « Choisir une image » ouvre la fenêtre de sélection du système, filtrée sur les images PNG, JPEG ou WebP (un second filtre propose tous les fichiers).
- [ ] `fond.png` : le fond de toute l'application change aussitôt (écran affiché, navigation, barre de titre) ; le panneau indique « IMAGE PERSONNALISÉE », le nom du fichier, la taille et les dimensions (ex. « 2,4 Mo · 3840 × 2160 »), et la vignette.
- [ ] Même chose avec `fond.jpg` puis `fond.webp`.
- [ ] Fichiers suivis de données après leur fin (`cat fond.png lourd.png | head -c 9000000 > png-suivi.png`, idem avec `fond.jpg`) : acceptés.
- [ ] Le dossier de configuration contient un fichier `background` ; `preferences.json` contient le nom du fichier, **jamais son chemin**.
- [ ] Déplacer ou supprimer l'image d'origine puis relancer : le fond est toujours là.

### Refus

Après chaque refus : encadré « Image refusée · … » avec le nom du fichier, **le fond en place ne change pas** et `background` n'est pas modifié.

- [ ] `lourd.png` : « fichier trop volumineux », « pèse N Mo ; la limite est de 10 Mo ».
- [ ] `capture.gif` (filtre « Tous les fichiers ») : « format non pris en charge ».
- [ ] `gif-renomme.png` et `texte.png` : « format non pris en charge » (le contenu fait foi, pas l'extension).
- [ ] `coupee.jpg` : « image illisible ».
- [ ] `abimee.png` : « image illisible ».
- [ ] `geante.png` : « dimensions trop grandes », avec ses dimensions et la limite (8 192 px de côté et 40 mégapixels). L'application reste fluide (l'image n'est pas décodée).
- [ ] Fermer la fenêtre de sélection sans choisir : rien ne change, aucun message.
- [ ] Un nouveau choix fait disparaître l'encadré du refus précédent.

### Assombrissement et couleurs

- [ ] Le curseur se déplace à la souris et au clavier (flèches, Origine, Fin) ; le pourcentage, le fond et l'aperçu en direct suivent aussitôt.
- [ ] 0 % : image nette ; 100 % : image masquée (fond uni de l'application).
- [ ] Relancer : la valeur choisie est conservée (enregistrée 300 ms après le dernier mouvement).
- [ ] « Couleurs d'origine » : l'image (fond, vignette, aperçu) passe en couleurs ; décoché, elle revient désaturée. Conservé au redémarrage.
- [ ] L'aperçu en direct montre la navigation et une liste de fichiers sur l'image, avec l'assombrissement en cours.

### Rétablissement et copie perdue

- [ ] « Rétablir le fond d'origine » : fond fourni avec Atlas, « FOND D'ORIGINE », bouton désactivé (contour pointillé) ; `background` est supprimé du dossier de configuration.
- [ ] Image personnalisée en place, Atlas fermé : supprimer `background` à la main, relancer. Le fond d'origine s'affiche, avec la notification « L'image enregistrée par Atlas est introuvable ou illisible. Atlas a repris le fond d'origine. » ; `preferences.json` ne référence plus d'image. Relancer encore : plus de notification.
- [ ] Même chose en remplaçant `background` par un fichier texte, puis par `coupee.jpg` renommé `background`.

### Persistance

- [ ] Image et réglages conservés après redémarrage, écran de connexion compris (le fond personnalisé s'y affiche).
- [ ] Après réinstallation par-dessus (même version ou version suivante) : conservés.
- [ ] Windows : désinstaller **sans** cocher la suppression des données puis réinstaller : conservés. En cochant la suppression : retour aux valeurs par défaut.
- [ ] Linux (.deb) : `sudo apt remove atlas-vps` puis réinstallation : conservés (`~/.config/dev.poutoo.atlas/` n'est pas supprimé).

### Rendu

- [ ] Windows (WebView2) et Linux (WebKitGTK) : les deux colonnes des préférences tiennent à 1024 px de large (taille minimale de la fenêtre), sans chevauchement ; les zones de texte restent sur des surfaces opaques.
- [ ] Parallaxe au changement de section toujours présente avec une image personnalisée ; absente en mouvement réduit.

## 4. Discord Rich Presence

Avec l'application installée (build release) et un `CLIENT_ID` valide. Vérifier la carte sur un **autre** compte ou appareil Discord, ou sur son propre profil (clic sur son avatar).

- [ ] Premier lancement : interrupteur éteint, « DÉSACTIVÉ », encadré « Désactivé · Rien n'est partagé avec Discord. », aperçu « APERÇU · NON PUBLIÉ » en pointillés. Aucune carte sur Discord.
- [ ] Discord lancé, activer l'interrupteur : « ACTIF · CONNECTÉ » en quelques secondes, encadré « Connecté à Discord », aperçu « PUBLIÉ SUR DISCORD ». Sur Discord : « Atlas · VPS Control », « Au sol », la durée écoulée et le papillon.
- [ ] Établir la liaison : la carte passe à « En liaison », durée repartie de zéro.
- [ ] Couper le réseau du VPS (LOS) : « Au sol » à la détection de la perte ; rétablir : « En liaison » de nouveau.
- [ ] « Couper la liaison » : « Au sol ».
- [ ] Enchaîner rapidement plusieurs connexions et déconnexions : aucune erreur, la carte finit sur l'état réel (5 mises à jour par 20 secondes au plus ; un changement peut arriver avec quelques secondes de retard).
- [ ] Désactiver l'interrupteur : « DÉSACTIVÉ » et la carte disparaît de Discord.
- [ ] Réactiver, puis fermer Atlas (bouton Fermer) : la carte disparaît.
- [ ] Atlas tué (Gestionnaire des tâches, `kill -9`) : Discord efface la carte de lui-même.
- [ ] Interrupteur actif, Discord fermé, lancer Atlas : « ACTIF · DISCORD ABSENT », encadré « Discord non détecté » avec « Réessayer », aperçu « EN ATTENTE DE DISCORD ». **Aucune notification d'erreur.**
- [ ] Lancer Discord ensuite : la carte apparaît en 15 secondes au plus, ou tout de suite avec « Réessayer ».
- [ ] Fermer Discord (quitter complètement) pendant qu'Atlas tourne : « Discord non détecté », sans notification ; relancer Discord : la carte revient seule.
- [ ] Préférence conservée au redémarrage (interrupteur actif, carte publiée au lancement).
- [ ] **Confidentialité** : sur la carte affichée par Discord (profil, liste des membres, infobulle du papillon), seuls apparaissent le nom « Atlas · VPS Control », « En liaison » ou « Au sol », la durée et le papillon. Jamais le nom du profil, l'hôte, l'adresse IP, l'utilisateur, un nom de fichier, de service ou de conteneur, une commande.
- [ ] Interrupteur éteint depuis le lancement : vérifier qu'Atlas ne se connecte pas à Discord (Windows : Process Explorer, onglet des handles d'Atlas, aucun `\Device\NamedPipe\discord-ipc-*` ; Linux : `ss -xp | grep atlas-vps` sans `discord-ipc`).
- [ ] L'aperçu « En liaison / Au sol » change l'aperçu seulement ; il montre la durée réelle pour l'état en cours, 00:00:00 pour l'autre.

### Linux : Flatpak et Snap (si possible)

- [ ] Discord en Flatpak (`flatpak install flathub com.discordapp.Discord`) : le socket est dans `$XDG_RUNTIME_DIR/app/com.discordapp.Discord/discord-ipc-0` ; la carte s'affiche.
- [ ] Discord en Snap (`snap install discord`) : socket dans `$XDG_RUNTIME_DIR/snap.discord/discord-ipc-0` ; la carte s'affiche.
- [ ] Noter ici tout autre emplacement observé : …

## 5. Mode développement

- [ ] `npm run tauri dev` : panneau « INDISPONIBLE », interrupteur désactivé (contour pointillé), encadré « Indisponible en mode développement · La présence Discord n'existe que dans l'application installée. », aperçu « APERÇU · INDISPONIBLE ». Aucune connexion à Discord, même si `discordPresence` vaut `true` dans `preferences.json`.
- [ ] Même chose avec `npm run tauri build -- --debug --no-bundle` (build debug).

## 6. Sans identifiant d'application

- [ ] Build release avec `CLIENT_ID` vide : « INDISPONIBLE », encadré « Indisponible dans cette version », interrupteur désactivé, aucune connexion à Discord.
