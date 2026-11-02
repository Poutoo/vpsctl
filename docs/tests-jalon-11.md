# Tests manuels du jalon 11 : projets docker compose

À dérouler sur Windows puis sur Linux, avec un build local (`npm run tauri build`) installé, sauf mention contraire. Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure.

Maquettes de référence : `docs/design/docker-compose/` (1280 et 1024 px). Règles : `docs/04-contrat-commandes.md`, « Docker compose ».

## 0. Vérification préalable des sorties réelles sur le VPS

À faire **avant tout le reste**, en SSH sur le VPS avec le compte de l'application. L'application repose sur ces formats : si une sortie diffère, s'arrêter et le signaler.

- [y] Version du plugin, sans « v » :

  ```sh
  docker compose version --short; echo "code $?"
  ```

  Attendu : `5.3.1` puis `code 0`.
- [y] Options globales de sortie en texte simple (sur n'importe quel projet existant, ici `nominal` de la préparation) :

  ```sh
  docker compose --ansi never --progress plain -p nominal restart 2>&1 | cat -A | head
  ```

  Attendu : des lignes de texte (`Container nominal-web-1 Restarting`, `… Started`) sans séquence `^[[` et sans ligne réécrite.
- [y] Étiquettes lues dans le même appel que la liste :

  ```sh
  docker ps -a --no-trunc --format '{"project":{{json (.Label "com.docker.compose.project")}},"configFiles":{{json (.Label "com.docker.compose.project.config_files")}},"envFiles":{{json (.Label "com.docker.compose.project.environment_file")}},"oneoff":{{json (.Label "com.docker.compose.oneoff")}}}' | head -3
  ```

  Attendu : un objet JSON par ligne ; `configFiles` contient les chemins **absolus** séparés par des virgules (projet `multi` : deux chemins) ; `envFiles` vaut `""` sauf pour le projet `envfile` ; `oneoff` vaut `"False"` (ou `"True"` pour un conteneur de `docker compose run`) ; `""` pour un conteneur hors compose.
- [y] Étiquette des volumes anonymes (Docker 29) :

  ```sh
  docker volume ls --filter label=com.docker.volume.anonymous --format '{{.Name}}'
  ```

  Attendu : les volumes anonymes du projet `anon` (64 caractères hexadécimaux), aucun volume nommé (`anon_named`, `nominal_data`…).
- [y] `restart` et `down` sans fichier : `cd /tmp && docker compose -p nominal restart; echo "code $?"` redémarre les conteneurs de `nominal` et donne `code 0`.

## 1. Préparation

Dans `~/atlas-tests/` sur le VPS, un dossier par projet, chacun lancé une fois avec `docker compose up -d` depuis son dossier (sauf mention contraire). Images légères (`busybox:1.37`).

| Projet | Contenu | Lancement |
|---|---|---|
| `nominal` | 2 services `sleep 1d`, un volume nommé `data` | `up -d` |
| `partial` | `web` (`sleep 1d`) et `migrate` (`command: "true"`, `restart: "no"`) | `up -d` |
| `failed` | `web` (`sleep 1d`) et `job` (`sh -c "exit 3"`) | `up -d` |
| `oom` | `web` (`sleep 1d`) et `hog` (`sh -c "tail /dev/zero"`, `mem_limit: 32m`) | `up -d` |
| `loop` | `web` (`sleep 1d`) et `crash` (`sh -c "sleep 2; exit 1"`, `restart: always`) | `up -d` |
| `multi` | `compose.yaml` (service `web`) et `compose.prod.yaml` (ajoute `worker`) | `up -d -f compose.yaml -f compose.prod.yaml` |
| `envfile` | `web` avec `command: sleep ${DELAY}`, `.env.prod` qui contient `DELAY=1d` | `--env-file .env.prod up -d` |
| `anon` | `db` avec `volumes: [named:/var/data, /cache, ./uploads:/uploads]` | `up -d` |
| `profiled` | `web` (`sleep 1d`) et `debug` (`profiles: [debug]`) | `--profile debug up -d` |

Plus un conteneur hors compose : `docker run -d --name kuma busybox:1.37 sleep 1d`.

## 2. Tests automatisés

### Windows

- [y] `npm ci` puis `npm test` : tous les tests passent (dont `composeView`, `composeActions`, `compose`, `composeRuns`, `serverSummary`).
- [y] `cd src-tauri && cargo test` : tous les tests passent (dont `compose::labels`, `compose::memory`, `compose::check`, `compose::command`, `compose::down`, `list_format_produces_one_json_object`).
- [y] `npm run lint`, `npm run typecheck`, `npx prettier --check .`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` : rien à signaler.
- [y] `npm run tauri build` réussit.

### Linux

- [ ] Mêmes commandes, mêmes résultats.

## 3. Détection du plugin

- [y] Barre d'état : « Ubuntu 26.04 LTS · Docker 29.6.1 · Compose 5.3.1 · systemd 259 ».
- [y] Plugin absent (sur un serveur de test sans plugin, ou en renommant temporairement `~/.docker/cli-plugins/docker-compose` s'il est installé pour l'utilisateur) : reconnexion, barre d'état « Compose absent » ; avis au-dessus du tableau « Le plugin docker compose n'est pas installé sur <profil>. » (maquette 09) ; les groupes restent affichés d'après les étiquettes, **sans** actions de projet ; les conteneurs gardent leurs actions. Remettre le plugin en place.

## 4. Groupes et états

- [y] Section Docker : « docker ps -a · 9 projets compose · 1 hors compose » (nombres réels) ; les projets triés par nom, puis « Hors compose » (titre en italique, « 1 CONTENEUR · sans étiquette com.docker.compose.project »), toujours en dernier.
- [y] Projet nominal : « NOMINAL · 2/2 », cercle plein ; dossier `/home/<compte>/atlas-tests/nominal` avec l'icône de dossier ; colonne Détail `compose.yaml`, chemins complets en infobulle.
- [y] Projet partiel : « PARTIEL · 1/2 », forme cible (cercle et point), ambre ; `migrate` « ARRÊTÉ · code 0 ».
- [y] Tous arrêtés proprement : `docker compose -p nominal stop`, puis Actualiser : « ARRÊTÉ · 0/2 », cercle vide (pas « Partiel »). Relancer avec Démarrer.
- [y] Projet en échec : « EN ÉCHEC · 1/2 », triangle, en-tête teinté de rouge ; `job` « EN ÉCHEC · code 3 ».
- [y] Manque de mémoire : `oom` « EN ÉCHEC », alors que `hog` est sorti avec le code 137 ; la ligne `hog` porte la mention MÉMOIRE et la limite de 32 Mio en infobulle.
- [y] Boucle de redémarrage : `loop` « EN ÉCHEC » (le conteneur `crash` passe par `restarting` avec le code 1) ; aucun clignotement vers « En transition » entre deux actualisations.
- [y] Un conteneur `docker compose run` (`cd ~/atlas-tests/nominal && docker compose run -d web sleep 600`) apparaît dans le groupe avec la mention RUN, mais le décompte reste « 2/2 ».
- [y] Repli : le chevron replie et déplie le groupe ; le repli est gardé en changeant d'écran.
- [y] Filtre `atlas-tests/multi` : seul le groupe `multi`, en entier. Filtre `worker` : seul le conteneur `multi-worker-1`, sous son en-tête.
- [y] Fenêtre étroite (1024 px) : colonne « État · Détail » sur deux lignes (maquette 1024/01).

## 5. Démarrer, Récupérer, Redémarrer

- [y] Infobulle de Redémarrer : « Redémarrer le projet <nom> » en gras, puis « Relance les conteneurs sans appliquer les changements du fichier compose ni les nouvelles images. Pour les appliquer : Démarrer. » (maquette 10).
- [y] Démarrer sur `partial` : l'écran Télémétrie s'ouvre sur l'onglet « up · partial », surtitre « TÉLÉMÉTRIE · COMMANDES COMPOSE », commande `docker compose --ansi never --progress plain -p 'partial' --project-directory '…' -f '…/compose.yaml' up -d` et « dans <dossier> » ; lignes `err` (progression de compose) ; fin `exit 0` ; notification « Projet démarré · partial · exit 0 · <durée> ».
- [y] Up long avec téléchargement : supprimer l'image (`docker image rm postgres:17` après avoir arrêté ce qui l'utilise), puis Démarrer sur un projet qui l'utilise. Pendant le téléchargement : onglet avec chrono T+, lignes de progression en texte simple (aucun code de terminal, aucune ligne réécrite) ; retour au tableau de bord : en-tête « UP · T+… », détail « docker compose up · en cours », conteneurs arrêtés en « DÉMARRAGE T+… · via docker compose up » (maquette 03), actions du projet et de ses conteneurs indisponibles sauf les logs ; navigation « ◐ 1 » à côté de Tableau de bord. Aucun arrêt au bout de 30 s : la commande va jusqu'au bout.
- [y] Échec d'un pull : dans `failed`, remplacer l'image de `job` par `ghcr.io/atlas-test/inexistant:latest`, puis Récupérer les images. Fin `exit 1` (maquette 04), bannière « FIN · EXIT 1 » ; notification d'erreur persistante avec « Voir la sortie » ; colonne Détail « pull · exit 1 · voir → » en rouge (maquette 05), qui ouvre l'onglet ; la marque disparaît au lancement de la commande suivante sur ce projet.
- [y] Seul le code de sortie juge : une commande réussie dont toute la sortie est sur `err` est notée en succès.
- [y] Redémarrer sur `nominal` : commande `… -p 'nominal' restart`, **sans** `-f` ni `--project-directory`, « par le nom du projet ».
- [y] Une commande à la fois par projet : pendant un up, cliquer Récupérer sur le même projet est impossible (« commande compose en cours sur ce projet »). Une commande sur un autre projet part normalement.
- [y] Après chaque commande, la liste et la vérification des fichiers sont relues (état du projet à jour sans attendre).

## 6. Plusieurs fichiers, fichier d'environnement, profil

- [y] `multi` : détail « compose.yaml + compose.prod.yaml » ; Démarrer passe `-f '…/compose.yaml' -f '…/compose.prod.yaml'` dans cet ordre ; `worker` reste créé.
- [y] `envfile` : Démarrer passe `--env-file '…/.env.prod'` ; le conteneur garde `sleep 1d`. Infobulle du détail : la ligne `--env-file`.
- [y] `profiled` (limite documentée) : le profil n'est enregistré dans aucune étiquette. Démarrer ne lance que `web` ; `debug`, lancé avec le profil, n'est ni recréé ni arrêté par Démarrer. Le noter dans le résultat.

## 7. Dossier supprimé, fichier illisible

- [y] Dossier supprimé : `mv ~/atlas-tests/nominal ~/atlas-tests/nominal.bak`. Au plus 30 s plus tard (ou sur Actualiser) : en-tête teinté, chemin barré avec l'icône de dossier en pointillés, détail « ▲ dossier introuvable », ligne d'explication (maquette 06) ; Démarrer et Récupérer indisponibles avec la raison, Redémarrer et Arrêter et supprimer disponibles.
- [y] Restart sur ce projet : réussit (`-p 'nominal' restart`).
- [y] Down sur ce projet : le GO / NO GO indique « Relance impossible depuis Atlas : … n'existe plus sur le serveur. » ; GO : les conteneurs et le réseau sont supprimés, le projet passe « Descendu · 0/2 ». Remettre le dossier (`mv … nominal`) et Démarrer.
- [y] Fichier illisible : `chmod 000 ~/atlas-tests/envfile/.env.prod`. Détail « ▲ fichier illisible », explication « …/.env.prod est illisible pour le compte connecté. » ; Démarrer et Récupérer indisponibles. Lancer Démarrer juste avant la vérification suivante (fichier rendu illisible entre-temps) : refus « fichier illisible pour le compte connecté : … », aucun onglet ne reste. `chmod 644` ensuite.
- [y] La vérification des fichiers n'a pas lieu à chaque actualisation de 5 s (pas de requête SFTP visible dans `journalctl` côté sshd à ce rythme, ou en observant le délai d'apparition : jusqu'à 30 s).

## 8. Arrêter et supprimer

- [y] GO / NO GO sans volume anonyme (`nominal`) : « ARRÊTER ET SUPPRIMER LES CONTENEURS DU PROJET ? », objet `nominal · <dossier>`, commande exacte ; à droite du bandeau « DONNÉES HORS VOLUMES PERDUES » (et non « Réversible par up ») ; liste : conteneurs supprimés (nombre, en marche, services), réseau `nominal_default` supprimé, volume nommé `nominal_data` conservé, images conservées, relance possible avec le fichier ; message : « Les conteneurs seront recréés par Démarrer et les volumes nommés sont conservés, mais les données écrites hors des volumes nommés sont perdues. » Focus sur NO GO, Échap = NO GO.
- [y] GO / NO GO avec volumes anonymes (`anon`) : ligne « 1 volume anonyme détaché : conservé sur le serveur, mais plus rattaché au projet après le down » avec son nom raccourci ; dossier monté `…/anon/uploads` listé comme conservé ; volume `anon_named` conservé.
- [y] Après le down de `anon` : `docker volume ls` contient toujours `anon_named` **et** le volume anonyme ; `docker ps -a --filter label=com.docker.compose.project=anon` est vide ; le projet est « Descendu · 0/1 ».
- [y] Service retiré du fichier : dans `multi`, retirer `worker` de `compose.prod.yaml`, puis Arrêter et supprimer. Après le down, `docker ps -a --filter label=com.docker.compose.project=multi` est **vide** (aucun conteneur `worker` orphelin). Remettre `worker`.

## 9. Projet descendu, oubli

- [y] Après un down, le projet reste affiché « Descendu · 0/N », cercle vide, détail « aucun conteneur · mémorisé », ligne « Aucun conteneur. Docker ne connaît plus ce projet… » (maquette 08) ; Démarrer et Récupérer disponibles, Redémarrer indisponible (« aucun conteneur ») ; l'icône Oublier remplace la suppression.
- [y] Démarrer sur le projet descendu : il est recréé depuis son dossier et redevient « Nominal ».
- [y] Persistance : fermer Atlas, le relancer, se reconnecter : le projet descendu est toujours là. Avec un autre profil (autre serveur), il n'apparaît pas.
- [y] Oublier ce projet : GO / NO GO simple (rien n'est touché sur le serveur) ; GO : le projet disparaît, notification « Projet oublié » ; `ls ~/atlas-tests/<projet>` et `docker volume ls` inchangés.
- [y] Supprimer un profil (déconnecté) : ses projets mémorisés disparaissent de `compose-projects.json` (dossier de configuration d'Atlas).

## 10. Interruption, perte de liaison

- [y] Interrompre un up en cours (bouton de la console) : GO / NO GO « Interrompre docker compose up ? », qui prévient que le projet peut rester à moitié démarré ; NO GO : la commande continue ; GO : bannière d'arrêt, colonne Détail « up · interrompue · voir → », liste relue.
- [y] Fermer l'onglet d'une commande en cours : même GO / NO GO.
- [y] Couper la liaison ou fermer l'application pendant une commande : GO / NO GO qui compte la commande compose en cours.
- [y] Perte de liaison pendant un up long (couper le réseau du poste) : LOS ; l'onglet porte « ISSUE INCONNUE » et « Liaison perdue · issue inconnue » ; aucune notification d'échec ; la colonne Détail « up · issue inconnue » ; rien n'est relancé au rétablissement. Après « Rétablir la liaison », la liste montre l'état réel (sur le serveur, la commande a été arrêtée par le script de surveillance quand sshd a fermé le canal).
- [y] Pendant le LOS, Relancer est indisponible ; après le rétablissement, Relancer relance la même commande.

## 11. Limites des onglets et des flux

- [y] 4 flux : ouvrir 3 onglets de logs en plein écran, lancer un up long, puis une commande sur un autre projet : refus « 4 flux déjà ouverts : fermez un onglet de télémétrie pour lancer cette commande », aucun onglet ne reste.
- [y] 6 onglets compose : lancer 7 commandes courtes (Récupérer) sur des projets différents, en laissant chacune finir : 6 onglets compose au plus ; à la septième, l'onglet compose terminé le plus ancien est fermé. Les onglets de logs et de scripts ne sont pas comptés.
- [y] Relancer la même commande sur un projet remplace son onglet terminé (pas de second onglet « pull · <projet> »).

## 12. Rendu

- [ ] Sous Windows (WebView2) et Linux (WebKitGTK) : formes SVG (disque, cible, triangle, demi-disque, cercle, coche, point d'exclamation du GO / NO GO), aucun caractère symbole ; trait de rattachement des conteneurs à l'en-tête de leur groupe ; infobulles à deux lignes.
