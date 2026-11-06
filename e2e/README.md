# Tests E2E (Windows)

WebdriverIO pilote l'application réelle par `tauri-driver` et `msedgedriver`
([doc Tauri](https://v2.tauri.app/develop/tests/webdriver/)). Les specs
traduisent `docs/tests-jalon-N.md`, un dossier par jalon (`specs/jalon-11/`,
`specs/jalon-12/`) et un fichier par section.

Les tests ne se connectent **jamais** au VPS : ils visent une cible Docker
locale (`e2e/target/`), avec une configuration d'Atlas séparée.

## Prérequis

- Docker Desktop démarré (la cible tourne en conteneur privilégié).
- `cargo install tauri-driver --locked`
- `ssh-keygen` (OpenSSH de Windows).
- msedgedriver : téléchargé automatiquement dans `e2e/.cache/`, à la version
  exacte du runtime WebView2 installé (lue dans le registre).

## Commandes

| Commande                                                   | Effet                                    |
| ---------------------------------------------------------- | ---------------------------------------- |
| `npm run test:e2e`                                         | build debug de test, puis tous les tests |
| `npm run e2e:build`                                        | build seul (`src-tauri/target/e2e/`)     |
| `npm run e2e:run`                                          | tests seuls, sur le dernier build        |
| `npm run e2e:run -- --spec e2e/specs/jalon-11/05-*.e2e.ts` | une section                              |
| `npm run e2e:run -- --spec e2e/specs/jalon-12/*.e2e.ts`    | un jalon                                 |
| `npm run e2e:typecheck`                                    | typage des specs                         |

Le build est sauté quand le binaire existe et qu'aucun fichier qui le compose
n'a changé depuis le dernier build réussi : fichiers suivis et nouveaux
fichiers non ignorés, hors `docs/`, `e2e/` (sauf `tauri.e2e.conf.json` et
`scripts/build.ts`), `.github/`, `.vscode/` et les `.md` de la racine.
`E2E_FORCE_BUILD=1` reconstruit quand même. Le frontend y est construit par
`vite build` seul : le typage (`npm run typecheck`) reste un critère de fin à
part.

`E2E_KEEP_TARGET=1` garde la cible démarrée après les tests (relances plus
rapides) ; `docker compose -f e2e/target/compose.yaml down -v` la supprime avec
ses données. `E2E_SSH_PORT` change le port local (2222 par défaut).

## Isolation

- Build avec `e2e/tauri.e2e.conf.json` : identifiant `dev.poutoo.atlas.e2e`,
  donc profils, projets mémorisés et données WebView2 dans
  `%APPDATA%\dev.poutoo.atlas.e2e` et `%LOCALAPPDATA%\dev.poutoo.atlas.e2e`,
  recréés à chaque fichier de spec.
- Le lanceur refuse un binaire sans cet identifiant, et tout profil dont
  l'hôte n'est pas la boucle locale.
- Trousseau : le service est l'identifiant de l'application, donc
  `dev.poutoo.atlas.e2e` ; les passphrases d'Atlas ne sont jamais lues. La clé
  SSH de test n'a d'ailleurs pas de passphrase (`e2e/.cache/ssh/`) : rien n'est
  écrit dans le trousseau.

## Projets compose (jalon 11)

`e2e/target/seed.sh`, lancé en root par les specs (`seed()`), remet les
projets de `e2e/target/projects/` dans l'état du tableau « Préparation » de
`docs/tests-jalon-11.md`. Chaque projet semé laisse une empreinte (conteneurs
et leur état, montages, réseaux, volumes, dossier du projet et sa copie
`.bak` : chemins, droits, propriétaires, contenus) ; au semis suivant, seuls
les projets dont l'empreinte a changé sont recréés. Le lancement de la suite
sème tout (`seedFull()`), et `seed("nominal")` recrée un projet sans
vérification. Les empreintes vivent dans le conteneur, hors du volume
Docker : une cible recréée repart d'un semis complet.

## Fichiers de l'éditeur (jalon 12)

`e2e/target/seed-editor.sh`, lancé en root par les specs (`seedEditor()`),
recrée `~/atlas-tests/editeur/` (fichiers de la « Préparation » de
`docs/tests-jalon-12.md`, plus `autre.conf`, propriété du second compte
`autre`, sans connexion possible) et `~/.ssh/authorized_keys` : la clé de test,
une seconde clé, une ligne avec options, un certificat signé par une autorité
de test et une ligne illisible (clés jetables, générées dans la cible). Les
conflits sont provoqués en modifiant les fichiers dans la cible (`sh`) pendant
l'édition ; chaque test d'`authorized_keys` remet le fichier en place, pour que
la clé de test reste autorisée.

## Ressources du serveur (jalon 13)

`e2e/target/seed-resources.sh`, lancé en root par les specs
(`seedResources()`), prépare la section 1 de `docs/tests-jalon-13.md` : volume
`atlas-gros` aux nombreux petits fichiers (rempli une fois, gardé entre deux
lancements ; il allonge l'analyse des volumes), conteneurs `atlas-limite`
(256 Mo) et `atlas-libre` (sans limite). Il rétablit aussi l'état normal
après un test : accès au socket Docker, disque de test démonté, conteneur de
mémoire et processus de charge arrêtés.

Les projets de test sont copiés dans l'image de la cible (`COPY projects`)
plutôt que montés depuis le poste : un dossier monté (9p) apparaîtrait comme
un disque du serveur, avec l'occupation du disque de Windows. Le seul vrai
disque de la cible est le volume `docker-data`, monté sur `/var/lib/docker` :
c'est le point de montage de la racine Docker.

## Écarts avec le test manuel

- « Up long avec téléchargement » : projet `slow` (attente d'un healthcheck de
  40 s, image `busybox:1.36` retirée avant) au lieu de `postgres:17`. Les
  autres up longs (une commande à la fois, interruption, demi-disque du rendu)
  ramènent cette attente à 15 s (`setSlowDelay`) ; la perte de liaison garde
  40 s, l'up devant encore tourner à la relance qui suit le gel.
- Perte de liaison : la cible est gelée (`docker pause`) au lieu de couper le
  réseau du poste.
- Section 2 (tests automatisés) et rendu Linux : marqués `skip`.
- Jalon 12 : fichier d'un autre compte `autre.conf` (compte `autre` de la
  cible) au lieu de `/etc/hosts` ; compte de la liaison `atlas`. Restent
  manuels, avec leur raison dans `docs/tests-jalon-12.md` : Ctrl+S au vrai
  clavier (WebDriver injecte les touches dans la page, sans passer par les
  raccourcis du moteur web), la fenêtre d'enregistrement de « Télécharger »,
  la saisie avec touches mortes et tout le rendu sous Linux.
- Jalon 13 : alertes provoquées en abaissant les seuils depuis l'interface ;
  charge montée par des processus `yes` dans la cible ; mémoire montée hors de
  l'écran par un conteneur qui remplit un tmpfs ; disque indisponible par un
  système de fichiers monté en boucle sous un dossier fermé (test ignoré si
  l'hôte Docker refuse le montage) ; hystérésis de la mémoire observée 30 s
  au lieu de 2 minutes (règle couverte par les tests Rust de `next_level`).
  Restent manuels, avec leur raison dans
  `docs/tests-jalon-13.md` : CPU, mémoire, charge et réseau indisponibles (le
  noyau fournit toujours leurs fichiers), la fenêtre réduite et tout le rendu
  sous Linux.
