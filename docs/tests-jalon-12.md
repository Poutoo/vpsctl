# Tests manuels du jalon 12 : édition de fichiers texte

À dérouler sur Windows puis sur Linux, avec un build local (`npm run tauri build`) installé, sauf mention contraire. Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure.

Maquettes de référence : `docs/design/editeur/` (1280 et 1024 px). Règles : `docs/04-contrat-commandes.md`, « Édition de fichiers ».

Les tests E2E (`e2e/specs/jalon-12/`) automatisent ces cas sous Windows, contre la cible Docker locale ; ceux qui restent manuels sont marqués **Manuel**, avec la raison.

## 0. Préparation

En SSH sur le VPS, avec le compte de l'application (ici `deploy`), dans `~/atlas-tests/editeur/` :

```sh
mkdir -p ~/atlas-tests/editeur && cd ~/atlas-tests/editeur
printf 'NODE_ENV=production\r\nPORT=8080\r\nLOG_LEVEL=info\r\n' > crlf.env
printf '\xef\xbb\xbfnom=été\n' > bom.txt
printf 'a\r\nb\nc\r\n' > mixte.txt
{ head -c 10000 /dev/zero | tr '\0' 'a'; printf '\n\xc3\x28 fin\n'; } > utf8-invalide.txt
printf 'SQLite format 3\0' > binaire.db
head -c 1200000 /dev/zero | tr '\0' 'x' > gros.log
ssh-keygen -q -t ed25519 -N '' -C jetable -f cle.pem && rm cle.pem.pub
ln -s crlf.env lien.env
printf 'version: 1\n' > lecture-seule.txt && chmod 444 lecture-seule.txt
mkdir verrouille && printf 'A=1\n' > verrouille/reglages.env && chmod 555 verrouille
printf 'A=1\nB=2\nC=3\n' > droits.conf && chmod 640 droits.conf
printf 'A=1\nB=2\nC=3\nD=4\nE=5\n' > conflit.env
printf 'A=1\nB=2\nC=3\nD=4\nE=5\nF=6\nG=7\n' > diff.env
printf 'un\ndeux\ntrois\nquatre\ncinq\n' > notes.txt
```

| Fichier | Sert à |
|---|---|
| `crlf.env` | fins de ligne CRLF conservées, résumés par noms de clés |
| `bom.txt` | BOM conservé |
| `mixte.txt` | lecture seule : fins de ligne mélangées |
| `utf8-invalide.txt` | refus : UTF-8 invalide après les 8 premiers Ko |
| `binaire.db` | refus : octet nul à la position 15 |
| `gros.log` | refus : plus de 1 Mo |
| `cle.pem` | refus : clé privée hors de `~/.ssh` (clé jetable, sans usage) |
| `lien.env` | refus : lien symbolique |
| `lecture-seule.txt` | lecture seule : droits 444 |
| `verrouille/reglages.env` | lecture seule : dossier 555 |
| `droits.conf` | droits 640 conservés |
| `conflit.env` | conflit |
| `diff.env` | différence |
| `notes.txt` | résumés par numéros de ligne |

Fichier d'un autre compte : `/etc/hosts` (propriétaire `root`). Avant les tests d'`authorized_keys` (section 9) : **garder une seconde session SSH ouverte** ou l'accès à la console de l'hébergeur, et une copie : `cp ~/.ssh/authorized_keys ~/authorized_keys.bak`.

## 1. Tests automatisés

### Windows

- [y] `npm ci` puis `npm test` : tous les tests passent (dont `textDiff`, `editorDiff`, `editorText`, `editorFacts`, `sshConfirmation`, `editor`).
- [y] `cd src-tauri && cargo test` : tous les tests passent (dont `editor::text`, `editor::access`, `editor::authorized_keys`, `editor::tests`).
- [y] `npm run lint`, `npm run typecheck`, `npx prettier --check .`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` : rien à signaler.
- [y] `npm run test:e2e` : toutes les specs passent (jalons 11 et 12).
- [y] `npm run tauri build` réussit.

### Linux

- [ ] Mêmes commandes, sauf `npm run test:e2e` (la suite E2E ne tourne que sous Windows), mêmes résultats.

## 2. Ouverture

- [y] Explorateur, dossier `editeur` : menu d'un fichier (clic droit ou « ⋯ ») : « Modifier » apparaît pour un fichier, pas pour un dossier. « Modifier » sur `crlf.env` : l'éditeur remplace la liste (maquette 01) ; surtitre « II · Itinéraire · Édition », titre `crlf.env`, itinéraire jusqu'à `crlf.env ÉDITION`.
- [y] Double-clic sur `notes.txt` dans la liste : même ouverture. Entrée sur un fichier sélectionné : même ouverture.
- [y] Bandeau « ÉDITION », à droite « aucune modification · ouvert à HH:MM:SS » ; barre du bas « Ligne 1, colonne 1 », « 3 lignes », « UTF-8 », « CRLF », taille, `-rw-r--r-- 644`.
- [y] Une étape de l'itinéraire (`editeur`) ou Fermer : retour à la liste, le fichier sélectionné.

## 3. Refus

Chacun avec son écran (maquettes 12 à 14) ; le contenu n'est jamais affiché.

- [y] `gros.log` : « REFUSÉ · TROP VOLUMINEUX », « Ce fichier dépasse la limite de l'éditeur. », taille 1,1 Mo en rouge, limite 1 Mo ; bouton « Télécharger 1,1 Mo ».
- [ ] **Manuel** (la fenêtre d'enregistrement du système n'est pas pilotable par WebDriver) : « Télécharger 1,1 Mo » ouvre cette fenêtre, puis le transfert apparaît dans la file.
- [y] `binaire.db` : « REFUSÉ · CONTENU BINAIRE », « premier octet nul à la position 15 (0x000f) », sans nom de format ; téléchargement proposé.
- [y] `utf8-invalide.txt` (octet invalide après les 8 premiers Ko) : « Ce fichier n'est pas de l'UTF-8 valide. », « première séquence invalide à l'octet 10 001 (ligne 2) » ; téléchargement proposé.
- [y] `cle.pem` (hors de `~/.ssh`) : « Une clé privée ne s'ouvre jamais dans l'éditeur. », « en-tête PRIVATE KEY à la ligne 1 », dossier `~/atlas-tests/editeur` ; **pas** de bouton Télécharger.
- [ ] Une clé privée de `~/.ssh` (`id_ed25519` s'il existe) : même refus, dossier « ~/.ssh · fichiers sensibles ».
- [y] `lien.env` : « Un lien symbolique ne s'ouvre pas dans l'éditeur. », cible `crlf.env` affichée ; pas de téléchargement.

## 4. Lecture seule

Bandeau « LECTURE SEULE », raison en tête du cadre (maquettes 19 à 21), contenu sélectionnable et copiable, pas de bouton Enregistrer, « Télécharger » proposé ; Ctrl+S sans effet.

- [y] `lecture-seule.txt` : « LECTURE SEULE · DROITS INSUFFISANTS », « L'utilisateur deploy n'a pas le droit d'écrire dans ce fichier. » ; « Droits -r--r--r-- 444 · propriétaire deploy:deploy · … dossier … inscriptible ».
- [y] `/etc/hosts` : « AUTRE PROPRIÉTAIRE », « Ce fichier appartient à root ; vous êtes connecté en deploy. … Atlas ne change pas le propriétaire d'un fichier. », puis « Aussi : dossier non inscriptible ».
- [y] `verrouille/reglages.env` : « DOSSIER NON INSCRIPTIBLE », « Le dossier … appartient à deploy et n'est pas inscriptible par deploy. Enregistrer écrit d'abord un fichier temporaire dans ce dossier, ce qui est impossible ici. »
- [y] `mixte.txt` : « FINS DE LIGNE MÉLANGÉES », barre du bas « CRLF + LF ».

## 5. Fidélité

- [y] `crlf.env` : remplacer `info` par `debug`, Ctrl+S. Sur le VPS, `od -c crlf.env | head` : toujours `\r \n` à chaque fin de ligne.
- [y] `bom.txt` : ajouter une ligne, enregistrer. `head -c 3 bom.txt | od -An -tx1` : `ef bb bf`. La barre du bas affiche « UTF-8 BOM ».
- [y] `droits.conf` : modifier, enregistrer. `stat -c %a droits.conf` : `640` ; `ls -la` : aucun fichier `.droits.conf.atlas-….part` restant.
- [y] Un fichier LF (`notes.txt`) reste en LF ; un fichier sans retour final reste sans retour final.

## 6. Enregistrement

- [y] À l'ouverture, « Enregistrer » est désactivé (contour pointillé) et le bandeau dit « aucune modification ». Taper puis effacer un caractère : de nouveau « aucune modification ».
- [y] Modifier `notes.txt` : « ● NON ENREGISTRÉ » à côté du titre, bandeau « ● MODIFIÉ · +1 −1 LIGNES », trait ambre dans la gouttière face aux lignes modifiées (maquette 02).
- [y] **Manuel** (WebDriver injecte les touches dans la page, sans passer par les raccourcis du moteur web) : Ctrl+S au clavier enregistre, sans fenêtre « Enregistrer sous » ni autre réaction du moteur web, sous Windows (WebView2) **et** sous Linux (WebKitGTK).
- [y] Pendant l'écriture : « ◐ ENREGISTREMENT · T+0,4 s » et « écriture d'un temporaire puis renommage » (maquette 04).
- [y] Après : « ✓ ENREGISTRÉ À HH:MM:SS » et « identique au serveur » (maquette 05), notification « Enregistré · notes.txt · 26 o · HH:MM:SS ». Une nouvelle frappe efface « identique au serveur ».

## 7. Conflit

Ouvrir `conflit.env`, changer `B=2` en `B=vous`, puis sur le VPS : `sed -i 's/D=4/D=serveur/' ~/atlas-tests/editeur/conflit.env`.

- [y] Ctrl+S : fenêtre « CONFLIT · CONFIRMATION REQUISE », « RIEN N'A ÉTÉ ÉCRIT » (maquette 06) : « Ouvert à … · modifié sur le serveur à … · 20 → 26 octets » ; « Écraser efface 1 ligne changée sur le serveur · D » ; « Recharger abandonne votre ligne modifiée · B » ; « Voir la différence ligne à ligne → ». Le fichier du VPS est inchangé.
- [y] Annuler (ou Échap) : retour à l'édition, texte intact.
- [y] Ctrl+S, puis « Recharger la version du serveur » : le texte devient celui du serveur (`D=serveur`, `B=2`), « aucune modification ».
- [y] Refaire le changement de `B`, provoquer un nouveau changement sur le serveur, Ctrl+S puis GO Écraser : le fichier du VPS contient votre version (`B=vous`) ; un nouveau changement serveur entre la fenêtre et le clic donne un nouveau conflit.
- [y] Supprimer le fichier sur le VPS pendant une édition, Ctrl+S : « Le fichier a été supprimé sur le serveur », GO Recréer : le fichier revient avec votre texte et ses droits.

## 8. Différence

Ouvrir `diff.env`, changer `D=4` en `D=vous` et ajouter `Y=1` en fin de fichier ; sur le VPS : `sed -i -e 's/A=1/A=serveur/' -e 's/D=4/D=serveur/' -e '/F=6/d' -e '/B=2/a Z=0' ~/atlas-tests/editeur/diff.env`. Ctrl+S, puis « Voir la différence ligne à ligne → » (maquette 07).

- [y] « DIFFÉRENCE · SERVEUR → VOUS · LECTURE SEULE » ; deux colonnes de numéros (serveur, vous) ; « − » sur fond ambre pour la version du serveur, « + » sur fond vert-de-gris pour la vôtre ; étiquettes SERVEUR (ambre) et VOUS ; lignes identiques repliées (« … 1 ligne identique »).
- [y] `A` : bloc SERVEUR, une seule action « Prendre la version du serveur ».
- [y] `Z=0` ajoutée sur le serveur : action « Reprendre ». `F=6` supprimée sur le serveur : action « Retirer ». `Y=1` : bloc VOUS, sans action.
- [y] `D` (vous et le serveur au même endroit) : bloc mixte, action « Prendre la version du serveur · efface votre ligne 4 » (contour rouge). La cliquer remplace votre ligne ; « Retour à l'édition » puis Ctrl+Z la rétablit.
- [y] Une fois toutes les lignes du serveur reprises : « Tous les changements du serveur sont repris : écraser ne perd plus rien. » ; « Enregistrer… » : GO / NO GO « Tous les changements du serveur sont repris », GO : enregistré, le fichier du VPS contient vos lignes et celles du serveur.
- [y] Résumés de la fenêtre de conflit : noms de clés pour `diff.env` (`A · Z · D · F`) ; pour `notes.txt` modifié des deux côtés, numéros de ligne (« lignes 2, 4–5 du serveur »).

## 9. authorized_keys et ~/.ssh

Seconde session SSH ouverte (voir Préparation).

- [y] `~/.ssh/config` (ou `known_hosts`) : bandeau permanent « FICHIER SSH », sans GO / NO GO à l'enregistrement.
- [y] `~/.ssh/authorized_keys` : bandeau « FICHIER D'ACCÈS SSH · Une erreur ici peut vous empêcher de vous reconnecter à <profil>, Atlas compris. … » (maquette 08).
- [y] Ajouter un commentaire, Ctrl+S : « ACCÈS SSH · CONFIRMATION REQUISE », « ACCÈS CONSERVÉ » (maquette 09) : « La clé de cette liaison reste autorisée · ligne N » (algorithme, commentaire, fin du base64), « N clés reconnues », « Droits conservés · 600 ». GO désactivé tant que la case « J'ai un autre moyen d'accéder au serveur… » n'est pas cochée. GO : enregistré.
- [y] Ctrl+S de nouveau après une autre modification : la case est **décochée**.
- [y] Une ligne avec options (`from="10.0.0.1",no-pty ssh-ed25519 …`) et un certificat (`ssh-ed25519-cert-v01@openssh.com …`) comptent parmi les clés reconnues ; une ligne illisible (`ssh-ed25519 AAAA tronquée`) : « ligne N non reconnue par Atlas ».
- [y] Supprimer la ligne de la clé de la liaison, Ctrl+S : « Enregistrer sans la clé de cette liaison ? », « PERTE D'ACCÈS PROBABLE », encadré « LIAISON EN COURS MENACÉE », « La clé de cette liaison est retirée du fichier … ligne N à l'ouverture », case encadrée de rouge (maquette 10). NO GO : rien n'est écrit.
- [y] Sur le VPS, retirer la clé de la liaison du fichier (la liaison reste ouverte), rouvrir le fichier, ajouter une clé : « CLÉ ABSENTE AVANT ET APRÈS », constat neutre « Clé de cette liaison introuvable avant comme après », case requise (maquette 11). Restaurer : `cp ~/authorized_keys.bak ~/.ssh/authorized_keys`.

## 10. Perte de liaison

- [y] Pendant une édition non enregistrée, couper le réseau du poste : bandeau LOS de l'application, bandeau du cadre « ▲ LIAISON PERDUE · +1 −1 LIGNE », « texte conservé · enregistrement indisponible », cadre en pointillé, Enregistrer désactivé, texte toujours modifiable (maquette 15). Ctrl+S sans effet.
- [y] Rétablir le réseau puis « Rétablir la liaison » : notification « Liaison rétablie · AOS · <profil> · fichier à revérifier » (maquette 16) ; le texte est intact, Enregistrer redevient actif ; Ctrl+S enregistre si le fichier n'a pas changé, sinon la fenêtre de conflit s'affiche (modifier le fichier sur le VPS pendant la coupure pour le vérifier).

## 11. Sorties avec des modifications

Chaque fois : GO / NO GO avec « +N −N lignes modifiées seront perdues » (noms de clés ou numéros), « Le fichier sur le serveur reste tel quel · version du HH:MM:SS », NO GO « Rester », focus sur NO GO, Échap = NO GO.

- [y] Fermer : « Quitter sans enregistrer ? » (maquette 03). NO GO : on reste, texte intact. GO : retour à la liste.
- [y] Une étape de l'itinéraire, ou une autre section (Tableau de bord) : même fenêtre.
- [y] Couper la liaison : « Couper la liaison sans enregistrer ? », « La liaison avec <profil> sera coupée » (maquette 17).
- [y] Fermer la fenêtre (bouton de la barre de titre, Alt+F4) : « Fermer Atlas sans enregistrer ? » (maquette 18). NO GO : Atlas reste ouvert.
- [y] Sans modification : aucune fenêtre dans tous ces cas.

## 12. Clavier

- [y] Tab dans le texte insère une tabulation (le curseur reste dans la zone).
- [y] Échap puis Tab : le focus quitte la zone de texte (barre du bas « Tab : quitter la zone » après Échap) ; une autre touche entre les deux annule la sortie.
- [y] Ctrl+Z annule la tabulation insérée.
- [y] **Manuel** : saisie avec accents et touches mortes (`é`, `ê`) sous Windows et Linux.

## 13. Rendu

- [y] Fenêtre 1280 × 800 et 1024 × 680 (navigation réduite) : écrans conformes aux maquettes `editeur-1280` et `editeur-1024` ; à 1024 px, le bouton Enregistrer et les fenêtres GO / NO GO tiennent dans la largeur.
- [ ] **Manuel** sous Linux (WebKitGTK) : mêmes écrans, gouttière alignée sur les lignes, défilement fluide d'un fichier de près de 1 Mo, Ctrl+S (section 6).
