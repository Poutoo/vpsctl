# Atlas

Application de bureau pour piloter un VPS Linux par SSH : explorateur de fichiers (SFTP), conteneurs Docker, services systemd utilisateur et scripts personnalisés. Une seule connexion SSH, par clé, avec vérification de l'empreinte du serveur. Aucun serveur ni service en ligne : tout se passe entre l'application et le VPS.

Windows 10 et 11, Linux (paquet .deb ou AppImage). Interface en français.

Préparation du serveur (compte utilisé, groupe `docker`, lingering) : voir [docs/02-preparation-vps.md](docs/02-preparation-vps.md).

## Installation

Les fichiers sont attachés à chaque version, dans la page [Releases](https://github.com/Poutoo/vpsctl/releases).

### Windows

1. Télécharger `Atlas_<version>_x64-setup.exe` et le lancer.
2. L'installateur n'est pas signé : Windows affiche « Windows a protégé votre ordinateur » (SmartScreen). Cliquer sur **Informations complémentaires**, puis **Exécuter quand même**. Le navigateur peut aussi signaler un fichier peu téléchargé : choisir de le conserver.
3. L'installation se fait pour l'utilisateur courant, sans droits administrateur, dans `%LOCALAPPDATA%\Atlas\`. Atlas apparaît dans le menu Démarrer ; un raccourci sur le bureau est proposé à la fin.

Si le **Smart App Control** de Windows 11 est activé, il bloque les applications non signées sans possibilité de passer outre : l'installation n'est alors possible qu'en le désactivant (Sécurité Windows → Contrôle des applications et du navigateur).

### Linux : paquet .deb (Debian, Ubuntu et dérivées)

```sh
sudo apt install ./atlas-vps_<version>_amd64.deb
```

`apt` installe les dépendances (WebKitGTK 4.1, GTK 3). Atlas apparaît dans le menu des applications ; en terminal, la commande est `atlas-vps`.

Les passphrases sont gardées par le service Secret Service du bureau (GNOME Keyring, KWallet, KeePassXC…). Le paquet recommande `gnome-keyring` ; sans aucun de ces services, l'application fonctionne mais la passphrase est demandée à chaque connexion.

### Linux : AppImage (autres distributions)

```sh
chmod +x atlas-vps_<version>_amd64.AppImage
./atlas-vps_<version>_amd64.AppImage
```

L'AppImage a besoin de FUSE 2 (`libfuse2`, ou `libfuse2t64` sous Ubuntu 24.04 et suivantes) et d'un service Secret Service pour mémoriser les passphrases. Construite sous Ubuntu 24.04, elle demande une distribution au moins aussi récente (glibc 2.39).

### Mise à jour

Pas de mise à jour automatique : installer la nouvelle version par-dessus l'ancienne (installateur Windows ou `sudo apt install ./…deb`). Profils, scripts et passphrases sont conservés.

## Premier lancement

1. **Créer un profil** : sur l'écran de connexion, « Nouveau profil ». Renseigner le nom du profil, l'hôte, le port (22), l'utilisateur et le chemin de la clé privée (par exemple `~/.ssh/id_ed25519`). Un profil peut être marqué « par défaut » : il est présélectionné à l'ouverture.
2. **Établir la liaison** : choisir le profil, puis « Établir la liaison ».
3. **Empreinte du serveur** : à la première connexion, l'application affiche l'empreinte de la clé hôte. La comparer à celle relevée sur le serveur (`ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`, depuis la console de l'hébergeur), puis « L'empreinte correspond, accepter ». Elle est enregistrée dans le profil ; si elle change un jour, la connexion est bloquée et un écran d'alerte s'affiche.
4. **Passphrase de la clé** : si la clé en a une, elle est demandée. « Mémoriser dans le trousseau du système » (coché par défaut) évite de la ressaisir.
5. Le **tableau de bord** s'ouvre : conteneurs Docker et services systemd du compte. Les sections Fichiers et Scripts sont dans la navigation.

## Données stockées

| Donnée                                                                      | Windows                                                                                                  | Linux                                                                                         |
| --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Profils (`profiles.json`, empreintes comprises) et scripts (`scripts.json`) | `%APPDATA%\dev.poutoo.atlas\`                                                                            | `~/.config/dev.poutoo.atlas/`                                                                 |
| Passphrases des clés                                                        | Gestionnaire d'identification Windows, entrées `passphrase:<id du profil>` du service `dev.poutoo.atlas` | Trousseau Secret Service, mêmes entrées (`secret-tool search --all service dev.poutoo.atlas`) |
| Données du moteur web (cache)                                               | `%LOCALAPPDATA%\dev.poutoo.atlas\`                                                                       | `~/.local/share/dev.poutoo.atlas/`                                                            |

Les clés privées restent où elles sont : l'application lit seulement le chemin indiqué dans le profil. Aucun mot de passe ni aucune passphrase n'est écrit dans un fichier.

## Désinstallation

- **Windows** : Paramètres → Applications → Atlas → Désinstaller. Le désinstalleur propose de supprimer aussi les données de l'application (`%APPDATA%\dev.poutoo.atlas\` et `%LOCALAPPDATA%\dev.poutoo.atlas\`) ; sinon elles restent en place pour une réinstallation.
- **Linux (.deb)** : `sudo apt remove atlas-vps`. Les dossiers du tableau ci-dessus ne sont pas supprimés : les effacer à la main si besoin.
- **Linux (AppImage)** : supprimer le fichier, et les dossiers du tableau si besoin.

Dans tous les cas, les passphrases mémorisées restent dans le trousseau du système. Pour les retirer : Gestionnaire d'identification sous Windows (entrées `dev.poutoo.atlas`), Seahorse ou `secret-tool clear service dev.poutoo.atlas` sous Linux.

## Développement

Tauri 2 (Rust) et Vue 3 (TypeScript). Prérequis : Node 24, Rust (version figée par `rust-toolchain.toml`), et sous Linux les paquets de développement de WebKitGTK (voir `.github/workflows/ci.yml`).

```sh
npm ci
npm run tauri dev      # application en développement
npm run tauri build    # installateurs (src-tauri/target/release/bundle/)
```

Documentation du projet dans [docs/](docs/), à commencer par [docs/03-architecture.md](docs/03-architecture.md). Les icônes se régénèrent depuis `src/assets/logo/` avec `npm run icons`.

Publication : pousser un tag `vX.Y.Z` (même version dans `package.json`, `src-tauri/Cargo.toml` et `src-tauri/tauri.conf.json`). Le workflow `release.yml` construit les installateurs et les attache à une release en brouillon, à publier depuis GitHub.
