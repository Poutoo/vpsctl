# Tests E2E (Windows)

WebdriverIO pilote l'application réelle par `tauri-driver` et `msedgedriver`
([doc Tauri](https://v2.tauri.app/develop/tests/webdriver/)). Les specs
traduisent `docs/tests-jalon-11.md`, un fichier par section.

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
| `npm run e2e:typecheck`                                    | typage des specs                         |

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

## Écarts avec le test manuel

- « Up long avec téléchargement » : projet `slow` (attente d'un healthcheck de
  40 s, image `busybox:1.36` retirée avant) au lieu de `postgres:17`.
- Perte de liaison : la cible est gelée (`docker pause`) au lieu de couper le
  réseau du poste.
- Section 2 (tests automatisés) et rendu Linux : marqués `skip`.
