# Tests manuels du jalon 9 : filtre, activation au démarrage, manque de mémoire, papillon

À dérouler sur Windows puis sur Linux, avec un build local (`npm run tauri build`). Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure.

## 0. Vérification préalable des sorties réelles sur le VPS

À faire **avant tout le reste**, en SSH sur le VPS avec le compte de l'application. L'analyse de l'application repose sur ces formats : si une sortie diffère, s'arrêter et le signaler.

- [y] La page de manuel de `systemctl` (systemd 259) ne documente `--output=` que pour `status` ; c'est le code de `systemctl` qui l'applique aussi aux tableaux. Vérifier :

  ```sh
  systemctl --version | head -1
  systemctl --user list-unit-files --type=service --output=json --no-pager > /tmp/atlas-luf.json; echo "code $?"
  head -c 600 /tmp/atlas-luf.json; echo; grep -c 'unit files listed' /tmp/atlas-luf.json
  ```

  Attendu : `systemd 259`, puis un tableau JSON sur une seule ligne, de la forme `[{"unit_file":"api.service","state":"enabled","preset":"enabled"},…]` (`preset` vaut `null` pour les états qui n'en ont pas, par exemple `static`), et `code 0`. Pas de ligne « N unit files listed. ».
- [y] Aucun fichier correspondant : `systemctl --user list-unit-files --type=service --output=json --no-pager 'atlas-inexistant*.service'; echo "code $?"` affiche `[]` puis un code **non nul**. L'application accepte ce cas (le code n'est pas consulté).
- [y] Les services créés par Atlas apparaissent avec `"state":"enabled"` (ou `disabled` s'ils ont été désactivés), jamais `static` : `grep -L '^\[Install\]' ~/.config/systemd/user/*.service` ne liste aucun service créé par l'application.
- [y] Format de `docker inspect` utilisé pour le manque de mémoire (sur n'importe quel conteneur, ID complet) :

  ```sh
  docker inspect --type container --format '{{.Id}} {{.State.OOMKilled}} {{.HostConfig.Memory}}' -- "$(docker ps -aq --no-trunc | head -1)"
  ```

  Attendu : une ligne `<64 caractères hexadécimaux> false 0` (ou `true`, ou une limite en octets).
- [y] Lingering actif (nécessaire au test de redémarrage) : `loginctl show-user "$USER" --property=Linger` donne `Linger=yes`.

## 1. Tests automatisés

### Windows

- [y] `npm ci` puis `npm test` : tous les tests passent (dont `listFilter`, `FilterField`, `AppSwitch`, `startup`, `containerState`, `TitleBar`).
- [y] `cd src-tauri && cargo test` : tous les tests passent (dont les tests `list-unit-files`, `startup_block`, `set_startup`, `inspect`).
- [y] `npm run lint`, `npm run typecheck`, `npx prettier --check .`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` : rien à signaler.
- [y] `npm run tauri build` produit `Atlas_1.1.0_x64-setup.exe` ; l'application installée affiche la version 1.1.0 dans la barre d'état.

### Linux

- [ ] Mêmes commandes, mêmes résultats ; le build produit le .deb et l'AppImage en 1.1.0.

## 2. Filtre des listes

Préparation : au moins 4 conteneurs et 4 services, dont un nom avec majuscules (`Urbanflow-API`), une description accentuée (« Sauvegarde réseau »), et dans un dossier de l'explorateur des fichiers `deploy.sh`, `Été.txt`, `notes.md` et un dossier.

### Barre de recherche du moteur web

- [y] Sur **chaque écran** (Tableau de bord, Fichiers, Scripts, Télémétrie, Réglages, Création de service), Ctrl+F n'ouvre **jamais** la barre de recherche du moteur web (WebView2 sous Windows, WebKitGTK sous Linux). Sur les écrans sans filtre, rien ne se passe.
- [y] Même vérification en `npm run tauri dev`.

### Tableau de bord

- [y] Au repos, le champ « Filtrer les tableaux » est à gauche d'« Actualiser » et affiche `Ctrl` `F`.
- [y] Ctrl+F place le curseur dans le champ, même depuis un tableau, et sélectionne le texte déjà saisi.
- [y] `urban` : les deux tableaux sont filtrés en même temps ; la correspondance est surlignée (fond de sélection et soulignement ambre) dans les noms, images, unités et descriptions ; chaque en-tête passe à « 2 sur 4 » (nombres réels).
- [y] Majuscules : `URBAN` donne le même résultat que `urban`.
- [y] Accents : `reseau` trouve « Sauvegarde réseau », et `réseau` trouve `api-reseau.service`.
- [y] `nginx` (aucun conteneur ni service) : dans chaque tableau, une seule ligne sous les en-têtes, « 0 SUR 4 · Aucun résultat pour « nginx ». » et « Effacer le filtre », qui vide le champ. Ce n'est pas l'état vide habituel (« Aucun conteneur » / « Aucun service utilisateur »).
- [y] Rempli, le champ propose une croix qui le vide.
- [y] Échap dans le champ rempli le vide ; un second Échap rend le focus au tableau (le curseur quitte le champ).
- [y] Le filtre suit les rafraîchissements automatiques (un conteneur démarré ailleurs apparaît s'il correspond).
- [y] Aller dans Fichiers puis revenir : le texte du filtre est conservé.
- [y] Ctrl+F pendant une fenêtre GO / NO GO ouverte : le focus reste dans la fenêtre.

### Explorateur

- [y] Le champ « Filtrer le dossier » est à droite de l'itinéraire.
- [y] `.sh` : seul `deploy.sh` reste ; pied de liste « 1 sur N entrées ».
- [y] `ete` et `ÉTÉ` trouvent `Été.txt`.
- [y] `zzz` : une ligne « 0 SUR N · Aucun résultat… » sous les en-têtes, avec « Effacer le filtre ». Un dossier réellement vide affiche toujours « Dossier vide ».
- [y] Les flèches, Début, Fin et Entrée ne parcourent que les lignes affichées.
- [y] Ouvrir un autre dossier vide le filtre ; aller au Tableau de bord puis revenir sur le même dossier le conserve.
- [y] « Nouveau fichier » vide le filtre (l'élément créé reste visible).
- [y] Renommer une entrée filtrée sous un nom qui ne correspond plus au filtre : le filtre est vidé, l'entrée reste visible et sélectionnée.
- [y] Pendant un renommage, Ctrl+F ne quitte pas la saisie.
- [y] Un nom déjà pris par une entrée masquée par le filtre est refusé dès la saisie (« Un élément porte déjà ce nom. »).
- [y] Dossier de plusieurs milliers d'entrées (`mkdir -p ~/beaucoup && cd ~/beaucoup && touch f{1..5000}.txt`) : le filtre `f49` reste fluide.

### Perte de liaison et fin de session

- [y] Couper la liaison (pare-feu, voir `docs/tests-jalon-8.md`) avec un filtre actif : après le LOS, le filtre reste en place, et on peut le modifier, l'effacer, utiliser Ctrl+F et Échap sur les données figées (tableaux et explorateur).
- [y] Au rétablissement, le filtre est conservé et s'applique aux listes relues.
- [y] « Couper la liaison » puis se reconnecter : les deux filtres sont vides.

## 3. Activation au démarrage

Préparation : créer depuis Atlas un service `atlas-boot` (commande `/bin/sh -c 'while true; do date; sleep 30; done'`), démarré et donc activé.

### Activer, désactiver

- [y] La colonne « Au démarrage » est entre État et Description ; `atlas-boot.service` affiche l'interrupteur à droite, « ACTIVÉ ».
- [y] Cliquer : pendant le changement, curseur à mi-course, contour pointillé, « ◑ DÉSACTIVATION » ; puis « DÉSACTIVÉ ». Aucune fenêtre GO / NO GO, aucune notification.
- [y] Le service **tourne toujours** (état NOMINAL) : la désactivation ne l'arrête pas. Sur le VPS : `systemctl --user is-enabled atlas-boot.service` → `disabled`, `systemctl --user is-active atlas-boot.service` → `active`.
- [y] Réactiver : « ACTIVATION » puis « ACTIVÉ » ; `is-enabled` → `enabled`. Un service arrêté et activé n'est pas démarré par l'activation.
- [y] Au clavier : Tab jusqu'à l'interrupteur (anneau de focus visible), Espace ou Entrée le bascule.
- [y] Deux clics rapides : une seule commande, l'interrupteur reste « en cours » jusqu'au résultat.
- [y] Échec : sur le VPS, `chmod a-w ~/.config/systemd/user/default.target.wants` puis désactiver `atlas-boot` : notification d'erreur persistante « Échec de la désactivation au démarrage » avec le message de systemctl ; l'interrupteur revient à « ACTIVÉ ». Rétablir avec `chmod u+w ~/.config/systemd/user/default.target.wants`.

### Après un redémarrage du VPS (lingering)

- [y] `atlas-boot` activé, et un second service de test `atlas-boot-off` créé puis **désactivé** depuis Atlas (laissé démarré).
- [y] Redémarrer le VPS depuis la console Hetzner (« Redémarrer »), attendre son retour, « Rétablir la liaison ».
- [y] `atlas-boot.service` est NOMINAL et « ACTIVÉ » sans avoir été démarré à la main ; `atlas-boot-off.service` est ARRÊTÉ et « DÉSACTIVÉ ».
- [y] Sur le VPS, `systemctl --user status atlas-boot.service` montre un démarrage à l'heure du boot.

### États non modifiables

Chaque cas : contour pointillé, curseur grisé, cadenas et cause en capitales ; l'infobulle (survol et focus clavier) donne l'explication ; un clic ne fait rien.

| Préparation sur le VPS | Cause affichée |
|---|---|
| Fichier `~/.config/systemd/user/atlas-static.service` écrit à la main **sans** `[Install]`, puis `systemctl --user daemon-reload` | STATIQUE |
| `systemctl --user mask atlas-boot-off.service` (démasquer ensuite avec `unmask`) | MASQUÉ |
| Gabarit `~/.config/systemd/user/atlas-tpl@.service` (avec `[Install]`), puis `systemctl --user start atlas-tpl@1.service` | INSTANCE |
| Fichier hors du dossier lié : `systemctl --user link ~/units/atlas-link.service` | LIÉ |
| `systemctl --user unmask atlas-boot-off.service` puis `systemctl --user enable --runtime atlas-boot-off.service` (le désactiver ensuite avec `disable --runtime`) | ACTIVÉ (SESSION) |

- [ ] STATIQUE.
- [ ] MASQUÉ.
- [ ] INSTANCE.
- [ ] LIÉ (et le lien n'est jamais supprimé par l'application).
- [ ] ACTIVÉ (SESSION), curseur à droite.
- [ ] Facultatif : rendre la commande illisible est difficile à provoquer ; l'état « INCONNU » est couvert par les tests automatisés.

### Perte de liaison

- [ ] Pendant un LOS : chaque interrupteur actif passe en contour pointillé, l'infobulle indique « liaison perdue », un clic ne fait rien.
- [ ] Couper au pare-feu juste après un clic sur l'interrupteur : aucune notification d'échec ; après le LOS, l'interrupteur montre la valeur figée. Au rétablissement, il montre l'état réel (vérifier avec `is-enabled`).

## 4. Conteneurs tués par manque de mémoire (OOMKilled)

### Préparation (commandes reproductibles)

Sur le VPS :

```sh
# Manque de mémoire : limite de 64 Mo sans swap, `tail` remplit la mémoire jusqu'au OOM.
docker run --name atlas-oom --memory 64m --memory-swap 64m alpine sh -c 'tail /dev/zero'

# Arrêt forcé sans manque de mémoire : même code de sortie 137.
docker run -d --name atlas-kill alpine sleep 3600
docker kill atlas-kill

# Contrôle : « 137 true » puis « 137 false ».
docker inspect -f '{{.State.ExitCode}} {{.State.OOMKilled}}' atlas-oom atlas-kill
```

Ne pas provoquer de manque de mémoire **sans** limite (`--memory`) : c'est le noyau du VPS entier qui serait à court de mémoire.

### Affichage

- [ ] `atlas-oom` : triangle « EN ÉCHEC » suivi de la mention encadrée « MÉMOIRE », ligne teintée de rouge, sans « code 137 ».
- [ ] Infobulle de son état : « Tué par manque de mémoire : le noyau a arrêté le processus à la limite de 64 Mo (OOMKilled). Code de sortie 137. » suivi de l'état brut.
- [ ] `atlas-kill` : cercle vide « ARRÊTÉ · code 137 » ; infobulle « Arrêté par SIGKILL, sans manque de mémoire : docker stop a dépassé son délai, ou kill -9. »
- [ ] Le compteur d'états de la section Docker compte `atlas-oom` parmi les échecs.
- [ ] Démarrer `atlas-oom` depuis Atlas : après la vérification à 2,5 s, notification d'erreur « Échec du démarrage · atlas-oom · code 137 · manque de mémoire ».
- [ ] Démarrer `atlas-kill` depuis Atlas : NOMINAL ; puis `docker kill atlas-kill` sur le VPS : au rafraîchissement suivant, de nouveau « ARRÊTÉ · code 137 ».
- [ ] Supprimer `atlas-kill` depuis Atlas : la ligne disparaît, aucune erreur (la liste n'a plus de 137 à inspecter pour lui).
- [ ] Pendant un LOS, `atlas-oom` garde « EN ÉCHEC · MÉMOIRE » dans les données figées ; au rétablissement, l'état est relu.
- [ ] Nettoyage : `docker rm atlas-oom atlas-kill`.

## 5. Barre de titre

- [ ] Le papillon (couleur, sans retouche) occupe toute la hauteur de la barre (32 px), suivi du mot « Atlas » en Garamond, puis « CONTRÔLE VPS ».
- [ ] Glisser la fenêtre en saisissant le papillon ou le mot : la fenêtre se déplace (aucune image fantôme). Double-clic sur le papillon : la fenêtre s'agrandit.
- [ ] Lecteur d'écran (Narrateur sous Windows, Orca sous Linux) : la marque est lue « Atlas », une seule fois (le papillon est décoratif).
- [ ] Largeurs de fenêtre, en AOS puis en LOS :

| Largeur | Attendu |
|---|---|
| 1280 × 800 | papillon, « Atlas », « CONTRÔLE VPS », indicateur AOS / LOS en entier |
| 1101 px | « CONTRÔLE VPS » encore visible ; navigation complète |
| 1100 px | « CONTRÔLE VPS » masqué, navigation réduite aux numéraux |
| 1024 × 680 | papillon et « Atlas » seuls à gauche ; l'indicateur LOS « LOS · <profil> · depuis mm:ss » tient en entier, sans chevaucher les boutons |

- [ ] 1280 × 800.
- [ ] 1101 et 1100 px.
- [ ] 1024 × 680, en AOS et en LOS.
- [ ] Mise à l'échelle Windows 125 % et 150 %, et écran HiDPI sous Linux : le papillon reste net.
- [ ] Thème du système : l'application n'a qu'un thème sombre et ne suit pas celui du système. Passer Windows (Paramètres → Personnalisation → Couleurs) puis GNOME (Paramètres → Apparence) en clair puis en sombre : la barre de titre ne change pas et le papillon reste lisible dans les deux cas.

## 6. Tableau de bord à 1024 px avec des noms longs

Préparation sur le VPS :

```sh
docker run -d --name urbanflow-worker-de-traitement-des-factures-nocturnes-1 \
  nginx:1.27-alpine sleep 86400
docker tag nginx:1.27-alpine registry.example.local/equipe-urbanflow/traitement-des-factures:2026.10.21-rc1
docker run -d --name atlas-image-longue registry.example.local/equipe-urbanflow/traitement-des-factures:2026.10.21-rc1 sleep 86400
```

Et, depuis Atlas, un service au nom de 64 caractères (`traitement-nocturne-des-factures-et-relances-clients-urbanflow-1`) avec une description de plus de 100 caractères. Garder `atlas-oom` (section 4) pour la mention MÉMOIRE.

- [ ] Fenêtre à 1024 × 680 : aucune barre de défilement horizontale ; les tableaux tiennent dans la largeur.
- [ ] Noms, images, unités et descriptions longs sont tronqués avec « … » ; le texte complet s'affiche en infobulle.
- [ ] Colonne État des conteneurs : « EN ÉCHEC » et « MÉMOIRE » tiennent en entier, sans retour à la ligne.
- [ ] Colonne « Au démarrage » : l'interrupteur et son étiquette (y compris « DÉSACTIVATION » et « ACTIVÉ (SESSION) ») tiennent dans ses 150 px ; seule la description rétrécit.
- [ ] Colonne Actions : les cinq icônes et le séparateur sont visibles, y compris avec un chrono T+ pendant une action.
- [ ] En-tête de page : le filtre (200 px), l'heure d'actualisation et « Actualiser » restent lisibles (retour à la ligne accepté, pas de chevauchement).
- [ ] Filtre `factures` : le surlignage reste visible dans les noms tronqués (au moins le début de la correspondance).
- [ ] Panneau de télémétrie ancré ouvert : mêmes constats.
- [ ] Nettoyage : supprimer les conteneurs et le service de test.
