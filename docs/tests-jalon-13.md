# Tests manuels du jalon 13 : ressources du serveur

À dérouler sur Windows puis sur Linux, avec un build local (`npm run tauri build`) installé, sauf mention contraire. Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure.

Maquettes de référence : `docs/design/planche VII/` (1280 et 1024 px). Règles : `docs/04-contrat-commandes.md`, « Ressources du serveur ». Écarts voulus avec les maquettes : la charge est affichée **par cœur** partout (valeur, seuils, courbe) ; la première lecture annonce « relevé 1 sur 2 » (deux relevés suffisent aux mesures par différence).

Les tests E2E (`e2e/specs/jalon-13/`) automatisent ces cas sous Windows, contre la cible Docker locale ; ceux qui restent manuels sont marqués **Manuel**, avec la raison. Pour déclencher une alerte, on abaisse les seuils depuis l'interface plutôt que de remplir un disque.

## 0. Vérification préalable des sorties réelles sur le VPS

À faire **avant tout le reste**, en SSH sur le VPS avec le compte de l'application. L'application repose sur ces formats : si une sortie diffère, s'arrêter et le signaler.

- [ ] Fichiers lus par SFTP, sans commande : `sftp vps` puis `get /proc/loadavg /tmp/loadavg` et `get /proc/stat /tmp/stat` : les fichiers locaux ne sont pas vides (le serveur SFTP lit `/proc` jusqu'à la fin malgré leur taille annoncée de 0).
- [ ] Extension `statvfs@openssh.com` : dans la même session `sftp`, `df -h /` affiche la taille et l'espace libre de `/`.
- [ ] Racine de Docker :

  ```sh
  docker info --format '{{.DockerRootDir}}'
  ```

  Attendu : un chemin absolu (`/var/lib/docker` en général).
- [ ] Conteneurs, sans flux continu :

  ```sh
  docker stats --no-stream --no-trunc --format '{{json .}}' | head -2
  ```

  Attendu : un objet JSON par conteneur en marche, avec `ID` (64 caractères hexadécimaux), `Name`, `CPUPerc` (`"4.80%"`) et `MemUsage` (`"1.2GiB / 3GiB"`, unités binaires). Un conteneur sans limite a pour limite la mémoire du serveur.
- [ ] Répartition par catégorie :

  ```sh
  docker system df --format '{{json .}}'
  ```

  Attendu : quatre lignes JSON, `Type` valant `Images`, `Containers`, `Local Volumes` et `Build Cache`, `Size` en unités décimales (`"14.2GB"`).
- [ ] Détail par volume :

  ```sh
  docker system df -v --format '{{json .}}' | head -c 600; echo
  ```

  Attendu : un seul objet JSON avec un tableau `Volumes` dont chaque élément a `Name` et `Size`.
- [ ] **Coût de la répartition** (après la préparation, volume `atlas-gros` en place) : `time docker system df >/dev/null` puis `time docker system df -v >/dev/null`, trois fois chacun. Noter les durées : `docker system df` ……… s ; `docker system df -v` ……… s. Si la première coûte presque autant que la seconde (80 % ou plus), la répartition ne doit plus partir d'elle-même : passer `DOCKER_USAGE_AUTOMATIC` à `false` (`src-tauri/src/resources/mod.rs`) ; l'alerte disque propose alors « Analyser l'espace Docker ». Les sections 11 et 12 indiquent ce qui change dans ce cas.

## 1. Préparation

Sur le VPS, avec le compte de l'application :

```sh
# Volume aux nombreux petits fichiers : allonge l'analyse des volumes.
docker volume create atlas-gros
docker run --rm -v atlas-gros:/d busybox:1.37 sh -c \
  'for i in $(seq 1 200); do mkdir /d/$i; for j in $(seq 1 1000); do : > /d/$i/$j; done; done'
# Un conteneur avec limite de mémoire, un sans limite, le premier lié au volume.
docker run -d --name atlas-limite --memory 256m -v atlas-gros:/d busybox:1.37 sleep 1d
docker run -d --name atlas-libre busybox:1.37 sleep 1d
```

Deux profils vers le même VPS (« VPS A » et « VPS B ») pour la section 10. À la fin des tests : `docker rm -f atlas-limite atlas-libre && docker volume rm atlas-gros`.

## 2. Tests automatisés

### Windows

- [ ] `npm ci` puis `npm test` : tous les tests passent (dont `resourceChart`, `resourceText`, `resourceRules`, `resources`).
- [ ] `cd src-tauri && cargo test` : tous les tests passent (dont `resources::proc`, `resources::containers`, `resources::alerts`, `resources::settings`, `resources::monitor`).
- [ ] `npm run lint`, `npm run typecheck`, `npx prettier --check .`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` : rien à signaler.
- [ ] `npm run test:e2e` : toutes les specs passent (jalons 11 à 13).
- [ ] `npm run tauri build` réussit.

### Linux

- [ ] Mêmes commandes, sauf `npm run test:e2e` (la suite E2E ne tourne que sous Windows), mêmes résultats.

## 3. Navigation

- [ ] La navigation compte cinq entrées : I Tableau de bord, II Fichiers, III Scripts, **IV Ressources**, **V Réglages**. Les surtitres des écrans suivent (« IV · Ressources », « V · … » pour les Réglages).
- [ ] Ressources n'est accessible qu'avec une liaison ; Réglages reste accessible sans liaison.

## 4. Nominal

- [ ] Ressources : titre « Ressources du serveur » ; à droite « Relevé HH:MM:SS · toutes les 5 s » et « 30 dernières minutes · en mémoire » (maquette 01). L'heure avance toutes les 5 s environ.
- [ ] CPU : pourcentage, nombre de cœurs, « pic N % sur 30 min », courbe d'utilisation avec la ligne des 100 %. Comparer avec `top` sur le VPS (même ordre de grandeur).
- [ ] Mémoire : utilisée en Go, « sur X Go · N % » (calculée sur `MemAvailable` : comparer avec la colonne `available` de `free -h`), ligne Swap ; courbe et seuils 85 % et 95 % en pointillés.
- [ ] Charge moyenne : valeur **par cœur** sur 5 min, puis « 1 min … · 15 min … », le nombre de cœurs et les seuils par cœur ; courbes 5 min et 1 min, seuils. Comparer avec `cat /proc/loadavg` divisé par `nproc`.
- [ ] Réseau : débits reçu et émis, interfaces comptées (ni `lo`, ni `docker0`, ni `veth*`, ni `br-*`), deux courbes.
- [ ] Espace disque : un point de montage par ligne, périphérique et type, barre avec les repères des deux seuils, « N Go libres sur M Go », état. Aucun `tmpfs`, `overlay`, `squashfs` (snap) ni doublon d'un même périphérique. Comparer avec `df -h -x tmpfs -x overlay -x squashfs -x devtmpfs` (même pourcentage, arrondi).
- [ ] Conteneurs : triés par mémoire décroissante, CPU en % d'un cœur, mémoire et limite (« 256 Mo » pour `atlas-limite`, « sans limite » pour `atlas-libre`), en-tête « docker stats ».
- [ ] Seuils d'alerte : profil affiché, disque 80 / 90 %, mémoire 85 / 95 %, charge 1 / 2 par cœur, explication de l'hystérésis.

## 5. Première lecture

- [ ] Établir la liaison puis ouvrir Ressources aussitôt : « Première lecture en cours… » dans l'en-tête, cartes en squelette avec « LECTURE » (maquette 06) ; CPU et réseau annoncent « relevé 1 sur 2 » jusqu'au deuxième relevé (5 s), mémoire, charge et disque s'affichent dès le premier.
- [ ] Quitter l'écran une minute puis y revenir : les courbes gardent leur historique (ligne interrompue pendant l'absence pour le CPU, la charge et le réseau, qui ne sont pas relevés hors de l'écran) ; CPU et réseau annoncent de nouveau « relevé 1 sur 2 » pendant 5 s.

## 6. Mesure indisponible

Une mesure impossible passe en « Indisponible », avec sa raison, sans arrêter les autres (maquette 07).

- [ ] Conteneurs : `sudo chmod 600 /var/run/docker.sock` sur le VPS : la section Conteneurs passe « Indisponible · accès à Docker refusé » ; CPU, mémoire, charge, réseau et disque restent à jour (l'heure du relevé avance). `sudo chmod 660 /var/run/docker.sock` : les conteneurs reviennent au relevé suivant.
- [ ] **Manuel, non reproductible sur un vrai serveur** (le noyau fournit toujours `/proc/stat`, `/proc/meminfo`, `/proc/loadavg` et `/proc/net/dev`) : CPU, mémoire, charge et réseau indisponibles, swap « non mesurable ». Couverts par les tests Rust des analyseurs (fichier illisible, champ manquant, aucune interface comptée) ; la carte Réseau indisponible se vérifie sur la maquette 07 uniquement.
- [ ] Disque : un point de montage dont `statvfs` est refusé passe « Indisponible » sur sa ligne, les autres restent affichés. **Manuel sur le VPS** (demande de monter un système de fichiers sous un dossier fermé, donc root) : automatisé contre la cible E2E.

## 7. Seuil franchi, pour chaque ressource

- [ ] Disque : seuil d'attention du disque abaissé sous l'occupation de `/` (par exemple 10) : bandeau « ATTENTION Disque bientôt plein · / · N % », ligne teintée, état ATTENTION ; critique abaissé aussi : bandeau « CRITIQUE Disque presque plein · … », barre rouge (maquettes 02 et 03). Le bandeau ne propose jamais d'aller vers un conteneur.
- [ ] Mémoire : seuils abaissés sous l'occupation : bandeau « Mémoire élevée · N % » avec swap et « Plus gros consommateur : <conteneur>, X Go » et le bouton « Voir <conteneur> » ; valeur et état de la carte en couleur.
- [ ] « Voir <conteneur> » : la page défile jusqu'à la ligne du conteneur dans le tableau Conteneurs, qui est surlignée quelques secondes.
- [ ] Charge : seuil d'attention de la charge abaissé à 0,1 (ou charge provoquée par `yes > /dev/null &` lancé autant de fois que de cœurs, puis `pkill yes`) : bandeau « Charge élevée · N par cœur », état de la carte.
- [ ] Ordre des bandeaux : disque, mémoire, charge. Valeurs par défaut restaurées : plus aucun bandeau.

## 8. Hystérésis

- [ ] Seuil d'attention du disque égal à l'occupation arrondie de `/` (ex. 41) : alerte levée. Seuil à 43 (2 points au-dessus) : l'alerte **reste**. Seuil à 45 (4 points au-dessus) : l'alerte retombe. Idem avec le seuil critique.
- [ ] Mémoire : seuil posé à 1 point sous l'occupation, puis laissé tel quel pendant 2 minutes : aucun clignotement de l'alerte quand la mémoire oscille autour du seuil.
- [ ] Charge : seuil au dixième près de la charge par cœur ; l'alerte ne retombe que 0,1 par cœur en dessous.

## 9. Repère de navigation

- [ ] Alerte disque ou mémoire active, aller dans Fichiers : l'entrée IV Ressources porte le repère (triangle pour attention, cercle à point d'exclamation pour critique, icônes SVG) ; à 1024 px, sur le chiffre IV.
- [ ] Depuis un autre écran, l'alerte suit la réalité dans la minute : rétablir les seuils par défaut depuis Ressources, aller dans Scripts, puis sur le VPS remplir un fichier jusqu'à franchir un seuil abaissé à l'avance (ou abaisser le seuil, quitter l'écran et attendre) : le repère apparaît en moins d'une minute sans revenir sur Ressources.
- [ ] La charge ne compte pas hors de l'écran : seule alerte active la charge, quitter Ressources : le repère disparaît.

## 10. Seuils propres à chaque profil, valeurs par défaut

- [ ] Profil « VPS A » : seuil disque 50 / 60. Couper la liaison, se connecter avec « VPS B » : seuils par défaut (80 / 90), historique vide. Revenir sur « VPS A » : 50 / 60 retrouvés.
- [ ] « Valeurs par défaut » sur « VPS A » : 80 / 90 / 85 / 95 / 1 / 2 ; les seuils de « VPS B » ne changent pas.
- [ ] Saisie invalide (attention ≥ critique, 0, 101, charge 0 ou au centième) : champ en erreur avec la raison, rien n'est enregistré, les alertes gardent les seuils précédents.
- [ ] Un seuil modifié s'applique tout de suite (sans attendre le relevé suivant) et est conservé après fermeture d'Atlas.

## 11. Répartition Docker

- [ ] Seuil du disque abaissé sous l'occupation du point de montage de la racine Docker : la ligne s'ouvre sur « Espace Docker · par catégorie » (images, conteneurs, volumes, cache de build, total) et « Analyser les volumes » (maquette 03).
- [ ] Sur le VPS, pendant ce temps : `watch -n 2 "pgrep -af 'docker system df'"` : la répartition ne repart pas plus d'une fois toutes les 5 minutes.
- [ ] Un autre point de montage en alerte (abaisser le seuil sous l'occupation de `/boot` si Docker n'y est pas) : aucune cause affichée.
- [ ] Si `DOCKER_USAGE_AUTOMATIC` vaut `false` (section 0) : à l'alerte, seul le bouton « Analyser l'espace Docker » est affiché ; il lance la répartition à la demande, sur le même modèle que l'analyse des volumes (section 12).

## 12. Analyse des volumes

- [ ] « Analyser les volumes » : « ANALYSE EN COURS », chemin et « peut durer plusieurs minutes. Les relevés continuent. », bouton Annuler (maquette 04). Les cartes continuent d'avancer.
- [ ] Fin : « Volumes · analyse de HH:MM:SS · durée », volumes triés par taille avec leurs conteneurs (`atlas-gros · atlas-limite`), « Relancer l'analyse » (maquette 05).
- [ ] Annuler pendant l'analyse : retour au bouton « Analyser les volumes ». Sur le VPS, `pgrep -af 'docker system df'` ne renvoie plus rien dans les 3 s. Limite connue : `dockerd` peut finir son parcours des volumes après l'arrêt de la commande (activité disque quelques instants).
- [ ] 4 flux déjà ouverts (onglets de logs) : l'analyse est refusée avec « 4 flux déjà ouverts : fermez un onglet de télémétrie pour lancer l'analyse ».
- [ ] Quitter l'écran pendant l'analyse puis revenir : l'analyse a continué (ou son résultat est affiché).

## 13. Perte de liaison

- [ ] Pendant les relevés (couper le réseau du poste) : LOS ; en-tête « Valeurs figées · dernier relevé HH:MM:SS », « Figé à HH:MM:SS » sous les valeurs, zone hachurée « sans relevé » qui s'élargit sur les courbes, bandeaux complétés par « Valeur figée : le serveur ne répond plus », repère de navigation conservé (maquette 08).
- [ ] « Rétablir la liaison » : les relevés reprennent, l'historique est gardé, la zone hachurée reste sur l'intervalle de la coupure ; CPU et réseau restent figés jusqu'au deuxième relevé.
- [ ] Pendant une analyse des volumes : à la perte, « Liaison perdue · analyse à relancer » ; après rétablissement, « Relancer l'analyse » fonctionne. Sur le VPS, la commande s'est arrêtée (script de surveillance).
- [ ] Seuils modifiables pendant le LOS ; les alertes figées sont recalculées.
- [ ] « Couper la liaison » : historique et alertes vidés ; reconnexion : première lecture.

## 14. Cadences

- [ ] Sur le VPS, `watch -n 1 "pgrep -af 'docker stats'"` : présent par moments quand Ressources est affiché, **jamais** quand on est sur un autre écran ou que la fenêtre est réduite.
- [ ] Hors de l'écran, la mémoire et le disque continuent d'être relevés chaque minute (point ajouté à la courbe de mémoire au retour) ; aucune commande `docker` n'est lancée par Atlas hors de l'écran.
- [ ] Deux relevés ne se chevauchent jamais : serveur ralenti (`stress`, ou liaison lente), l'heure du relevé avance par pas de 5 s **après** la fin du précédent, jamais deux `docker stats` à la fois.

## 15. Rendu

- [ ] Fenêtre 1280 × 800 : cartes sur une ligne, conforme aux maquettes `1280-*`.
- [ ] Fenêtre 1024 × 680 (navigation réduite) : cartes en 2 × 2, repère sur le chiffre IV, conforme aux maquettes `1024-*`.
- [ ] **Manuel** sous Linux (WebKitGTK) : mêmes écrans ; courbes, hachures et pointillés nets ; icônes SVG du repère et des états, aucun caractère symbole.
