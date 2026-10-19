# Tests manuels du jalon 7 : packaging et version 1.0.0

À dérouler sur Windows puis sur Linux, avec les installateurs de la release en brouillon (ou ceux d'un `npm run tauri build` local). Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure.

Fichiers :

- Windows : `Atlas_1.0.0_x64-setup.exe`
- Linux : `atlas-vps_1.0.0_amd64.deb` et `atlas-vps_1.0.0_amd64.AppImage`

## Préparation

- Fermer toute instance d'Atlas, y compris `npm run tauri dev`.
- **Mettre de côté les données existantes**, pour partir d'un dossier de configuration vide (elles sont remises en place en section 7) :
  - Windows : renommer `%APPDATA%\dev.poutoo.atlas` en `dev.poutoo.atlas.bak` et `%LOCALAPPDATA%\dev.poutoo.atlas` en `dev.poutoo.atlas.bak` ;
  - Linux : `mv ~/.config/dev.poutoo.atlas ~/.config/dev.poutoo.atlas.bak` et `mv ~/.local/share/dev.poutoo.atlas ~/.local/share/dev.poutoo.atlas.bak`.
- Noter les entrées `dev.poutoo.atlas` déjà présentes dans le trousseau (Windows : Gestionnaire d'identification → Informations d'identification Windows ; Linux : `secret-tool search --all service dev.poutoo.atlas`).
- Avoir sous la main l'empreinte du serveur (`ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`, relevée sur la console de l'hébergeur) et la passphrase de la clé.
- Garder un terminal SSH ouvert sur le VPS pour les vérifications côté serveur.

## 1. Installation propre

### Windows

- [y] Lancer `Atlas_1.0.0_x64-setup.exe` téléchargé depuis GitHub : SmartScreen affiche « Windows a protégé votre ordinateur ». « Informations complémentaires » fait apparaître « Exécuter quand même », qui lance l'installateur (procédure du README).
- [y] L'installateur est en français et ne demande **aucun** droit administrateur (pas de fenêtre du contrôle de compte d'utilisateur).
- [y] Dossier d'installation proposé : `%LOCALAPPDATA%\Atlas`. Il contient `atlas-vps.exe` et le désinstalleur.
- [y] Fin de l'installation : case « raccourci sur le bureau » proposée, et lancement possible.
- [y] Menu Démarrer : entrée « Atlas ». Paramètres → Applications : « Atlas », version 1.0.0, éditeur Poutoo.
- [y] Premier lancement : écran de connexion « Aucun profil pour l'instant ». `%APPDATA%\dev.poutoo.atlas` est vide ou absent (créé à la première sauvegarde).
- [y] Barre d'état : version 1.0.0.

### Linux : .deb

- [ ] `dpkg-deb -I atlas-vps_1.0.0_amd64.deb` : `Package: atlas-vps`, `Version: 1.0.0`, `Depends:` avec `libwebkit2gtk-4.1-0`, `libgtk-3-0` et `libdbus-1-3`, `Recommends: gnome-keyring | org.freedesktop.secrets`.
- [ ] `sudo apt install ./atlas-vps_1.0.0_amd64.deb` : installation sans erreur, dépendances résolues.
- [ ] `ldd /usr/bin/atlas-vps | grep "not found"` : aucune ligne.
- [ ] `/usr/share/applications/atlas-vps.desktop` : `Name=Atlas`, `Exec=atlas-vps`, `Icon=atlas-vps`.
- [ ] Menu des applications : entrée « Atlas » (recherche « Atlas » ou « VPS »). Lancement depuis le menu, puis depuis un terminal (`atlas-vps`).
- [ ] Premier lancement : écran de connexion « Aucun profil pour l'instant ». Barre d'état : version 1.0.0.

### Linux : AppImage

- [ ] `chmod +x atlas-vps_1.0.0_amd64.AppImage && ./atlas-vps_1.0.0_amd64.AppImage` : l'application démarre (sinon, installer `libfuse2t64` ou `libfuse2`).
- [ ] Mêmes profils, scripts et passphrases que la version .deb (même dossier de configuration et même trousseau).

## 2. Parcours complet depuis zéro

Sur l'application installée (pas en développement), dossier de configuration vide.

- [y] « Nouveau profil » : créer un profil (nom, hôte, port 22, utilisateur, `~/.ssh/id_ed25519`), le marquer par défaut, enregistrer. `profiles.json` apparaît dans le dossier de configuration.
- [y] « Établir la liaison » : panneau d'empreinte inconnue avec l'empreinte du serveur. Elle correspond à celle relevée ; « L'empreinte correspond, accepter ».
- [y] Passphrase demandée (« Déverrouiller la clé privée »), « Mémoriser dans le trousseau du système » coché : connexion établie. Une entrée `passphrase:<id>` du service `dev.poutoo.atlas` apparaît dans le trousseau.
- [y] Tableau de bord : conteneurs Docker et services systemd listés, rafraîchis toutes les 5 secondes. Barre d'état : système, Docker, systemd.
- [y] Panneau ancré : logs en direct d'un conteneur, puis d'un service ; « Plein écran » ouvre l'onglet Télémétrie sans couper le flux.
- [y] Redémarrer un service de test (GO / NO GO) : « En transition », puis « Nominal » et notification.
- [y] Fichiers : navigation, création d'un dossier et d'un fichier, renommage, envoi d'un fichier (fenêtre de sélection du système), téléchargement (fenêtre d'enregistrement, dossier Téléchargements proposé), « Afficher » ouvre l'explorateur local sur le fichier, suppression avec confirmation.
- [y] Scripts : créer un script (`uname -a`), l'exécuter : sortie en direct dans la Télémétrie, dernière exécution mémorisée. `scripts.json` apparaît dans le dossier de configuration.
- [y] Copier une empreinte ou une ligne de console : le presse-papiers fonctionne.
- [y] Barre de titre : déplacement, double-clic (agrandir / restaurer), réduire, agrandir, fermer (avec GO / NO GO si un script tourne).
- [y] Déconnexion puis reconnexion : aucune empreinte ni passphrase demandée.
- [y] Fermer et relancer l'application : le profil par défaut est présélectionné, la connexion n'exige rien.

## 3. Politique de sécurité du contenu (CSP)

La CSP n'est **pas** appliquée en `tauri dev` (la webview charge directement le serveur de Vite) : elle se vérifie sur un build qui embarque les fichiers. Voir docs/03-architecture.md, « Politique de sécurité du contenu ».

- [y] Mode développement (`npm run tauri dev`), inspecteur ouvert (Ctrl+Maj+I) : parcourir connexion, tableau de bord, logs, fichiers, scripts, réglages, création de service : aucune erreur ni avertissement dans la console.
- [y] Build debug avec la vraie CSP : `npm run tauri build -- --debug --no-bundle`, lancer `src-tauri/target/debug/atlas-vps` (`.exe` sous Windows), ouvrir l'inspecteur (Ctrl+Maj+I ou clic droit → Inspecter). Refaire le même parcours : aucune ligne « Content Security Policy » ni « Refused to » dans la console, sous Windows (WebView2) **et** sous Linux (WebKitGTK).
- [y] Dans ce build debug, onglet Réseau ou Console : les polices (EB Garamond, IBM Plex Sans et Mono) et l'image de fond sont chargées ; les appels IPC (`ipc.localhost` sous Windows, `ipc://` sous Linux) aboutissent.
- [y] Build installé (section 2) : toutes les fonctions du parcours marchent. Aucune zone sans style, aucune police de remplacement, console et liste des fichiers défilent correctement.

## 4. Build de production

- [y] Application installée : F12, Ctrl+Maj+I et clic droit n'ouvrent pas l'inspecteur (absent du build de publication).
- [y] F5, Ctrl+R, Ctrl+Maj+R : aucun effet. Clic droit hors champ de saisie : aucun menu du moteur web ; dans un champ : couper, copier, coller (comme au jalon 5).

## 5. Icônes

L'icône 16 px est dessinée à part (`atlas-icone-16.svg`) : à 16 px, elle doit être nette, pas une réduction floue de l'icône principale.

### Windows

- [y] Icône de `Atlas_1.0.0_x64-setup.exe` dans l'explorateur, et icône de la fenêtre de l'installateur.
- [y] Barre des tâches pendant l'exécution (et épinglée), Alt+Tab.
- [y] Menu Démarrer (liste « Toutes les applications » : icône 16 ou 24 px selon l'échelle), raccourci sur le bureau.
- [y] Explorateur, `%LOCALAPPDATA%\Atlas` en affichage « Petites icônes » et « Détails » : `atlas-vps.exe` affiche l'icône 16 px dédiée.
- [y] Paramètres → Applications : icône d'Atlas.
- [y] Contenu du .ico : `npm run icons` liste les entrées de `src-tauri/icons/icon.ico` ; l'entrée 16 × 16 provient de `atlas-icone-16.svg`.

### Linux

- [ ] Menu des applications et lanceur (dock, barre des tâches) : icône d'Atlas, y compris pendant l'exécution (la fenêtre est associée à `atlas-vps.desktop`, pas d'icône générique).
- [ ] `ls /usr/share/icons/hicolor/*/apps/atlas-vps.png` : `16x16`, `32x32`, `64x64`, `128x128`, `512x512` (et `256x256@2`, posé par Tauri, que les thèmes d'icônes ignorent).
- [ ] Écran haute densité (mise à l'échelle 200 %) : l'icône du menu des applications reste nette.
- [ ] Liste en petites icônes (gestionnaire de fichiers sur `/usr/share/applications`, ou sélecteur d'applications) : l'icône 16 px est la version dédiée.
- [ ] AppImage : icône dans le gestionnaire de fichiers si le bureau l'affiche, et dans la barre des tâches pendant l'exécution.

## 6. Réinstallation par-dessus une version existante

Avec les données de la section 2 (profil, empreinte, passphrase mémorisée, scripts).

- [y] Windows : relancer `Atlas_1.0.0_x64-setup.exe`. L'installateur détecte l'installation existante et propose de réinstaller : accepter. Au lancement, profil, empreinte, scripts (avec leur dernière exécution) et passphrase sont conservés : connexion sans rien ressaisir.
- [ ] Linux : `sudo apt install --reinstall ./atlas-vps_1.0.0_amd64.deb` : même résultat.
- [ ] Optionnel, avant la publication : installer un build 1.0.0 local, puis la release ; ou, plus tard, installer 1.0.1 par-dessus 1.0.0. Les données sont conservées et la nouvelle version s'affiche dans la barre d'état.

## 7. Désinstallation

### Windows

- [y] Paramètres → Applications → Atlas → Désinstaller : le désinstalleur propose de supprimer les données de l'application. **Ne pas cocher** : `%LOCALAPPDATA%\Atlas` et les raccourcis disparaissent ; `%APPDATA%\dev.poutoo.atlas` (profils, scripts) et `%LOCALAPPDATA%\dev.poutoo.atlas` (données du moteur web) restent. Le trousseau garde `passphrase:<id>`.
- [y] Réinstaller, puis désinstaller en **cochant** la suppression des données : `%APPDATA%\dev.poutoo.atlas` et `%LOCALAPPDATA%\dev.poutoo.atlas` sont supprimés. Le trousseau garde toujours `passphrase:<id>` (à retirer à la main dans le Gestionnaire d'identification, comme indiqué dans le README).
- [y] Rien d'autre ne reste : pas d'entrée dans le menu Démarrer ni dans la liste des applications.

### Linux

- [ ] `sudo apt remove atlas-vps` : `/usr/bin/atlas-vps`, `/usr/share/applications/atlas-vps.desktop` et les icônes `hicolor/*/apps/atlas-vps.png` sont supprimés ; l'entrée disparaît du menu.
- [ ] Restent : `~/.config/dev.poutoo.atlas/`, `~/.local/share/dev.poutoo.atlas/` et l'entrée du trousseau (`secret-tool search --all service dev.poutoo.atlas`).
- [ ] AppImage : supprimer le fichier ; les mêmes dossiers et l'entrée du trousseau restent.

### Remise en place

- [y] Supprimer les données de test et l'entrée de trousseau créée en section 2 (`secret-tool clear service dev.poutoo.atlas username passphrase:<id>` sous Linux, Gestionnaire d'identification sous Windows).
- [y] Remettre les dossiers `.bak` à leur nom d'origine, réinstaller la version du quotidien et vérifier la connexion habituelle.
