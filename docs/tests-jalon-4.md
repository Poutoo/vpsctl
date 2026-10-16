# Tests manuels du jalon 4 : explorateur SFTP

À dérouler sur Windows puis sur Linux. Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure.

Tous les objets de test sont dans le dossier `~/vpsctl-t-files` du VPS (et `~/vpsctl-t-cible`, la cible des liens), supprimés à la fin (section 16). Côté local, les fichiers de test sont préfixés `vpsctl-t-`.

> **Attention, section 9** : les tests d'accès SSH se font **uniquement avec NO GO**. Ne jamais valider (GO) une suppression, un renommage ou un écrasement de `~/.ssh` ou de `~/.ssh/authorized_keys`. Garder un terminal SSH ouvert pendant toute la procédure.

## Préparation

- Lancer l'application : `npm run tauri dev`, se connecter au VPS.
- Garder un terminal SSH ouvert sur le VPS, avec le même compte, pour les vérifications côté serveur.

### Cas de test sur le VPS

```sh
T=~/vpsctl-t-files
mkdir -p "$T"

# Cible des liens, hors du dossier de test : ne doit jamais être touchée
mkdir -p ~/vpsctl-t-cible
echo "ne pas toucher" > ~/vpsctl-t-cible/precieux.txt

# 1. Arborescence : sous-dossiers, fichiers cachés, liens vers l'extérieur, lien cassé
mkdir -p "$T/arbre/a/b" "$T/arbre/a/.cache" "$T/arbre/vide"
echo un > "$T/arbre/a/b/f.txt"
echo cache > "$T/arbre/a/.cache/x"
echo env > "$T/arbre/.env"
ln -s ~/vpsctl-t-cible "$T/arbre/vers-cible"
ln -s ~/vpsctl-t-cible/precieux.txt "$T/arbre/a/vers-fichier"
ln -s /nulle/part "$T/arbre/casse"
# Liens hors de l'arborescence : vers un dossier (à ouvrir), vers un fichier
# (à télécharger), vers le dossier personnel (accès SSH)
ln -s ~/vpsctl-t-cible "$T/lien-dossier"
ln -s ~/vpsctl-t-cible/precieux.txt "$T/lien-fichier"
ln -s ~ "$T/moi"

# 2. Fichier volumineux (500 Mo, aléatoire : la compression SSH ne fausse pas le débit)
head -c 500M /dev/urandom > "$T/gros-distant.bin"
sha256sum "$T/gros-distant.bin"          # noter l'empreinte

# 3. Écrasement
echo "version d'origine" > "$T/ecraser.txt"
chmod 640 "$T/ecraser.txt"
mkdir -p "$T/ecrasement" "$T/annulation"
echo "original, ne doit pas changer" > "$T/ecrasement/vpsctl-t-gros.bin"
sha256sum "$T/ecrasement/vpsctl-t-gros.bin"   # noter l'empreinte

# 4. Droits : dossier illisible, fichier illisible
mkdir -p "$T/interdit"
echo secret > "$T/interdit/s.txt"
chmod 000 "$T/interdit"
echo secret > "$T/illisible.txt"
chmod 000 "$T/illisible.txt"

# 5. Plusieurs milliers d'éléments
mkdir -p "$T/milliers"
(cd "$T/milliers" && for i in $(seq 1 5000); do : > "fichier-$i.txt"; done && mkdir dossier-1 dossier-2 dossier-10)

# 6. Résidus de transfert simulés (section 14)
echo "contenu partiel" > "$T/.rapport.pdf.atlas-0123abcd.part"
echo "ancienne version, seule copie" > "$T/.notes.txt.atlas-89abcdef.old"
echo "nouvelle version" > "$T/config.yml"
echo "ancienne version" > "$T/.config.yml.atlas-deadbeef.old"
```

### Fichiers locaux

Un fichier volumineux aléatoire de 500 Mo, `vpsctl-t-gros.bin`, et un petit fichier `ecraser.txt` (contenu « nouvelle version locale »).

Windows (PowerShell) :

```powershell
$dir = "$env:USERPROFILE\vpsctl-t-local"
New-Item -ItemType Directory -Force $dir | Out-Null
$rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
$buffer = New-Object byte[] (1MB)
$stream = [IO.File]::OpenWrite("$dir\vpsctl-t-gros.bin")
for ($i = 0; $i -lt 500; $i++) { $rng.GetBytes($buffer); $stream.Write($buffer, 0, $buffer.Length) }
$stream.Close()
Get-FileHash "$dir\vpsctl-t-gros.bin"     # noter l'empreinte
Set-Content -Path "$dir\ecraser.txt" -Value "nouvelle version locale"
```

Linux :

```sh
mkdir -p ~/vpsctl-t-local
head -c 500M /dev/urandom > ~/vpsctl-t-local/vpsctl-t-gros.bin
sha256sum ~/vpsctl-t-local/vpsctl-t-gros.bin    # noter l'empreinte
echo "nouvelle version locale" > ~/vpsctl-t-local/ecraser.txt
```

### Repérer les temporaires

Sur le VPS, en dehors des résidus simulés :

```sh
find ~/vpsctl-t-files -name '.*.atlas-*' ! -name '.rapport.pdf.atlas-0123abcd.part' ! -name '.notes.txt.atlas-89abcdef.old' ! -name '.config.yml.atlas-deadbeef.old'
```

En local, dans le dossier de destination des téléchargements :

```powershell
Get-ChildItem "$env:USERPROFILE\Downloads" -Force -Filter '.*atlas-*'
```

```sh
ls -A ~/Téléchargements ~/Downloads 2>/dev/null | grep 'atlas-'
```

## 1. Navigation et itinéraire

- [ ] Première ouverture de « Fichiers » après la connexion : le dossier personnel s'affiche (`/home/<compte>`), pas la racine.
- [ ] Itinéraire comme sur la capture `docs/design/ecran/fichiers/accueil.png` : « ○ vps-prod-01 / DÉPART », puis un nœud par dossier relié par un tracé pointillé, le dernier encadré en ambre avec « ● … POSITION ».
- [ ] Cliquer un nœud de l'itinéraire y revient. Cliquer la racine affiche `/` (nœud « DÉPART · POSITION »). Cliquer la position actuelle relit le dossier.
- [ ] Double-clic sur `vpsctl-t-files` y entre ; l'URL de la fenêtre (outils de développement) est `/files/home/<compte>/vpsctl-t-files`.
- [ ] Aller sur Tableau de bord puis revenir sur Fichiers : le dernier dossier visité est rouvert (et relu).
- [ ] Chemin long (`~/vpsctl-t-files/arbre/a/b`) dans une fenêtre étroite : l'itinéraire défile horizontalement, la position reste visible.
- [ ] Pied de liste : chemin complet à gauche, « N entrées » à droite ; pendant une lecture, « Lecture · T+… » (pas de spinner) ; bouton Actualiser.

## 2. Liste

- [ ] Dans `vpsctl-t-files/arbre` : `.env` et `a/.cache` sont affichés (fichiers cachés).
- [ ] Dossiers (et liens vers un dossier) en premier, puis les fichiers.
- [ ] Liens : icône de lien, cible affichée à droite du nom en texte discret (`→ /home/<compte>/vpsctl-t-cible`). `casse` affiche « → /nulle/part (cassé) » en rouge. La taille d'un lien et d'un dossier est « — ».
- [ ] `vers-cible` (lien vers un dossier) est rangé avec les dossiers ; double-clic ou Entrée y entre, et l'itinéraire montre `…/arbre/vers-cible`, pas la cible résolue. Le contenu affiché est celui de `~/vpsctl-t-cible`.
- [ ] Tri : clic sur Nom, Taille, Modifié, Droits. Une flèche ▲ / ▼ indique la colonne et le sens ; « ↕ » sur les autres. Un second clic inverse le sens. Les dossiers restent toujours en tête.
- [ ] Tri par nom numérique : dans `milliers`, `fichier-2.txt` vient avant `fichier-10.txt` ; `dossier-2` avant `dossier-10`.
- [ ] Droits au format `drwxr-xr-x`, `-rw-r-----` (`ecraser.txt`), `lrwxrwxrwx` (liens), `d---------` (`interdit`).
- [ ] Dates : « 12 oct. 14:02 » pour l'année en cours.
- [ ] Nom tronqué (réduire la fenêtre) : nom complet en infobulle.
- [ ] Sélection : fond ambré et trait de 2 px à gauche ; survol : fond plus clair.

### Fluidité avec des milliers d'éléments

- [ ] Ouvrir `milliers` (5 003 entrées) : la liste s'affiche en quelques secondes, « 5003 entrées ».
- [ ] Défilement à la molette et en faisant glisser la barre de défilement : fluide, sans ligne vide qui reste affichée.
- [ ] Fin puis Début au clavier : saut immédiat ; flèches maintenues : la sélection défile sans à-coup.
- [ ] Tri par taille puis par nom : réponse immédiate.
- [ ] Outils de développement, panneau Éléments : seules quelques dizaines de lignes existent dans le DOM.

## 3. Clavier

La liste a le focus (clic sur une ligne, ou Tab jusqu'à la liste : anneau de focus visible).

- [ ] Flèches haut / bas, Début, Fin, Pg↑, Pg↓ : la sélection se déplace et la ligne reste visible.
- [ ] Entrée sur un dossier ou un lien vers un dossier : y entre. Entrée sur un fichier : rien.
- [ ] Retour arrière : dossier parent, et le dossier quitté est sélectionné. À la racine : rien.
- [ ] F2 : renommage de la ligne sélectionnée (section 6). Suppr : suppression (section 7).
- [ ] Ctrl+D sur un fichier : fenêtre d'enregistrement (section 11). Sur un dossier : rien.
- [ ] Ctrl+Maj+C : notification « Chemin copié » ; coller dans un éditeur donne le chemin complet. En mode développement sous Windows, si Ctrl+Maj+C ouvre l'inspecteur des outils de développement, le noter (le raccourci est intercepté par WebView2 avant la page).
- [ ] Touche Menu (ou Maj+F10) : le menu de la ligne sélectionnée s'ouvre sous son bouton « ⋯ » ; flèches pour parcourir, Entrée pour choisir, Échap pour fermer. Après fermeture, le focus revient à la liste.
- [ ] Lecteur d'écran (Narrateur ou Orca) : la ligne sélectionnée est annoncée avec son nom et son genre (« dossier », « lien vers un dossier », « lien cassé »…).

## 4. Menu contextuel

- [ ] Clic droit sur une ligne : elle est sélectionnée et le menu s'ouvre au point de clic, comme sur `docs/design/ecran/menu-contextuel.png` : chemin en tête, Ouvrir (dossiers seulement), Télécharger (fichiers et liens vers un fichier seulement), Renommer (F2), Copier le chemin (Ctrl ⇧ C), séparateur, Supprimer en rouge.
- [ ] Pas d'entrée « Permissions… » ni « Modifier les droits ».
- [ ] Bouton « ⋯ » d'une ligne : même menu, sous le bouton. Infobulle « Actions de <nom> ».
- [ ] Survol ou flèches : fond plus clair et trait ambre ; Supprimer : trait rouge.
- [ ] Apparition : léger fondu avec un décalage de 4 px.
- [ ] Clic droit dans une zone vide de la liste : aucun menu (ni celui de l'appli, ni celui du système).
- [ ] Clic droit dans un champ de saisie (renommage) : menu du système (copier, coller).

## 5. Création

- [ ] « Nouveau fichier » : une ligne de saisie apparaît en tête de liste, comme sur `docs/design/ecran/fichiers/creation.png`, avec « ↵ créer · Échap annuler ». Le focus est dans le champ.
- [ ] Saisir `vpsctl-t-nouveau.txt`, Entrée : le fichier apparaît, sélectionné, taille « 0 o ». Sur le VPS, `ls -l` le confirme.
- [ ] « Nouveau dossier » : idem avec `vpsctl-t-dossier`.
- [ ] Noms refusés avant tout envoi, message rouge à la place de l'aide, champ en erreur : `a/b` (« / »), `.`, `..`, nom vide (Entrée sans rien saisir), un nom de plus de 255 octets.
- [ ] Nom déjà pris (`ecraser.txt`) : « Un élément porte déjà ce nom. » dès la saisie ; Entrée ne fait rien.
- [ ] Nom pris entre-temps : saisir `vpsctl-t-course`, créer `~/vpsctl-t-files/vpsctl-t-course` sur le VPS (`touch`), puis Entrée : message « Un élément porte déjà ce nom : … » renvoyé par le serveur ; le fichier existant n'est pas modifié.
- [ ] Échap : la ligne disparaît, rien n'est créé. Clic ailleurs : idem.
- [ ] Dans un dossier vide (`arbre/vide`) : « Nouveau fichier » fait apparaître la ligne de saisie à la place de l'état vide.
- [ ] Dans `interdit` (illisible) : les boutons de création et Téléverser sont désactivés (contour pointillé).

## 6. Renommage

- [ ] F2 sur `vpsctl-t-nouveau.txt` : le nom devient un champ, `vpsctl-t-nouveau` est présélectionné (sans l'extension). Saisir `vpsctl-t-renomme.txt`, Entrée : la ligne est renommée et reste sélectionnée.
- [ ] F2 sur un dossier : tout le nom est sélectionné.
- [ ] Renommer vers un nom existant (`ecraser.txt`) : refusé avec « Un élément porte déjà ce nom. » ; rien ne change sur le VPS.
- [ ] Même nom, Entrée : la saisie se ferme sans appel au serveur.
- [ ] Échap : nom inchangé, le focus revient à la liste.
- [ ] Renommer `lien-dossier` en `vpsctl-t-lien` : seul le lien est renommé ; `~/vpsctl-t-cible` est intact.

## 7. Suppression

- [ ] Suppr sur `vpsctl-t-renomme.txt` : GO / NO GO « Supprimer le fichier ? », chemin en ambre, « Le fichier (0 o) est supprimé définitivement », mention « Irréversible ». Focus sur NO GO ; Échap = NO GO ; GO rouge à droite.
- [ ] GO : la ligne disparaît, notification « Supprimé ».
- [ ] Lien `arbre/a/vers-fichier` : « Supprimer le lien ? », « Seul le lien est supprimé : sa cible … est conservée ». GO, puis sur le VPS : `cat ~/vpsctl-t-cible/precieux.txt` affiche toujours « ne pas toucher ».
- [ ] Dossier `arbre` (récursif) : la ligne passe « Décompte · T+… », puis GO / NO GO « Supprimer le dossier ? » avec « Contenu supprimé : 4 sous-dossiers, 3 fichiers, 2 liens symboliques · 13 o » (`vers-fichier` a été supprimé au test précédent), « Les liens symboliques sont supprimés seuls, jamais leur cible » et « Si un sous-dossier est illisible, rien n'est supprimé ».
- [ ] GO : « Suppression · T+… », puis la ligne disparaît. Sur le VPS :
  - `ls ~/vpsctl-t-files/arbre` : absent ;
  - `ls -la ~/vpsctl-t-cible` : `precieux.txt` toujours présent, contenu intact.
- [ ] NO GO après un décompte : rien n'est supprimé.
- [ ] Dossier contenant un sous-dossier illisible : `mkdir -p ~/vpsctl-t-files/mixte/ok ~/vpsctl-t-files/mixte/ferme && touch ~/vpsctl-t-files/mixte/ok/f && chmod 000 ~/vpsctl-t-files/mixte/ferme`. Supprimer `mixte` : le décompte échoue (« Décompte impossible », accès refusé). Sur le VPS, `ls -R ~/vpsctl-t-files/mixte` : tout est encore là. Puis `chmod 755 ~/vpsctl-t-files/mixte/ferme`.
- [ ] Dossier volumineux : supprimer `milliers` (5 003 éléments). Le décompte puis la suppression se terminent en quelques secondes (requêtes en parallèle), sans figer l'interface.
- [ ] Le dossier personnel et ses parents sont protégés : depuis `/home`, Suppr sur `<compte>` : notification d'erreur immédiate « Le dossier personnel et les dossiers qui le contiennent ne peuvent pas être supprimés. », sans décompte ni GO / NO GO. Même chose à la racine sur `home`. F2 sur `<compte>`, saisir un autre nom, Entrée : refusé de même (le backend le refuse aussi, voir section 15).

## 8. Erreurs

- [ ] Ouvrir `interdit` : état d'erreur « Lecture impossible · …/interdit », titre « Accès refusé », message « Accès refusé sur le VPS : … », boutons « Remonter » et « Réessayer ».
- [ ] Élément introuvable : afficher `vpsctl-t-files`, supprimer `ecraser.txt` depuis le VPS (`mv ~/vpsctl-t-files/ecraser.txt /tmp/`), puis Suppr dans l'appli sur sa ligne et GO : « Élément introuvable sur le VPS : … ». Remettre le fichier (`mv /tmp/ecraser.txt ~/vpsctl-t-files/`).
- [ ] Dossier disparu : `mkdir ~/vpsctl-t-files/ephemere`, l'ouvrir dans l'appli, `rmdir ~/vpsctl-t-files/ephemere` sur le VPS, puis Actualiser : « Dossier introuvable ».
- [ ] Élément déjà existant : sections 5 et 6.
- [ ] Les notifications d'erreur restent affichées jusqu'à leur fermeture.

## 9. Accès SSH

**NO GO uniquement.** Vérifier sur le VPS, après chaque test : `ls -la ~/.ssh` est inchangé.

- [ ] Dossier personnel, Suppr sur `.ssh` : le décompte s'affiche, puis le GO / NO GO contient l'avertissement « Cette action peut vous faire perdre l'accès SSH à ce serveur. » (triangle, fond rouge) et la case « Je comprends que je peux perdre l'accès SSH à ce serveur ». GO est désactivé (contour pointillé) tant que la case n'est pas cochée. Cocher, constater que GO s'active, puis **NO GO**.
- [ ] F2 sur `.ssh`, saisir `ssh-test`, Entrée : GO / NO GO « Renommer ? » avec l'avertissement et la case. **NO GO** : la saisie se ferme, le nom reste `.ssh`.
- [ ] Dans `~/.ssh`, Suppr sur `authorized_keys` : avertissement et case. **NO GO**.
- [ ] F2 sur `authorized_keys` : avertissement et case au moment de valider. **NO GO**.
- [ ] Écrasement : créer un fichier local nommé `authorized_keys` (contenu quelconque), Téléverser dans `~/.ssh` : GO / NO GO « Remplacer le fichier ? » avec l'avertissement et la case. **NO GO** : aucun transfert lancé.
- [ ] Pas d'avertissement pour `~/.ssh/known_hosts`, `~/.ssh/id_ed25519` ni pour un autre dossier du dossier personnel.
- [ ] Par un lien : ouvrir `~/vpsctl-t-files/moi` (lien vers le dossier personnel), Suppr sur `.ssh` : l'avertissement est présent. **NO GO**.

## 10. Envoi

- [ ] « Téléverser » (bouton ambre, flèche vers le haut) : fenêtre de sélection du système, plusieurs fichiers possibles, pas de dossier.
- [ ] Envoyer `vpsctl-t-gros.bin` (local) dans `vpsctl-t-files` : notification « Transfert lancé · vpsctl-t-gros.bin · LOCAL → vps-prod-01 ». Le panneau « TRANSFERTS · ORBITES » apparaît sous la liste, comme sur `docs/design/ecran/fichiers/televersement.png`.
- [ ] Pendant l'envoi : « ◐ N % », « 13,4 Mo / 500,0 Mo · 3,6 Mo/s · reste 02:16 » (largeur stable), le point ambre avance de LOCAL vers le serveur, le trajet parcouru est plein et le reste pointillé. Le point avance par petits glissements réguliers.
- [ ] Pendant l'envoi, sur le VPS : `ls -la ~/vpsctl-t-files` montre `.vpsctl-t-gros.bin.atlas-<8 hex>.part` qui grossit, et pas encore `vpsctl-t-gros.bin`.
- [ ] Fin : « ✓ ENVOYÉ », « 500 Mo · mm:ss », coche verte sur le nœud du serveur, notification « Envoyé ». La liste se relit : `vpsctl-t-gros.bin` apparaît, le `.part` a disparu. Empreinte : `sha256sum ~/vpsctl-t-files/vpsctl-t-gros.bin` identique à celle du fichier local.
- [ ] Mémoire : pendant l'envoi, la mémoire du processus de l'appli (gestionnaire des tâches, `top`) reste stable, sans croître avec la taille envoyée.
- [ ] **Annulation en cours d'envoi** : ouvrir `vpsctl-t-files/annulation`, y envoyer `vpsctl-t-gros.bin` ; à mi-parcours, Annuler. Le bouton affiche son chrono, puis « ANNULÉ », « Arrêté à … », point creux figé. Sur le VPS, aucun temporaire (commande « Repérer les temporaires ») et pas de `vpsctl-t-gros.bin` dans ce dossier.
- [ ] **Écrasement** : envoyer `ecraser.txt` (local) dans `vpsctl-t-files` : GO / NO GO « Remplacer le fichier ? » avec « Le fichier existant (18 o, modifié le …) est remplacé », « Le remplacement n'a lieu qu'à la fin de l'envoi… », « Les droits du fichier existant sont conservés ». NO GO : rien n'est envoyé. GO : envoi, puis `cat` donne « nouvelle version locale » et `ls -l` montre toujours `-rw-r-----` (640). Aucun `.part` ni `.old` restant.
- [ ] **Annulation d'un écrasement en cours** : envoyer `vpsctl-t-gros.bin` (local) dans `vpsctl-t-files/ecrasement`, GO pour remplacer, puis Annuler à mi-parcours. Sur le VPS : `sha256sum ~/vpsctl-t-files/ecrasement/vpsctl-t-gros.bin` identique à l'empreinte notée en préparation (contenu « original, ne doit pas changer ») ; aucun temporaire.
- [ ] Envoi sur un lien : créer un fichier local nommé `lien-dossier` (ou `vpsctl-t-lien` s'il a été renommé), l'envoyer dans `vpsctl-t-files` : refusé avant tout transfert, notification « Envoi refusé » avec « … est un lien symbolique vers /home/<compte>/vpsctl-t-cible : un envoi ne remplace jamais un lien. ». Le lien et sa cible sont intacts.
- [ ] Envoi sur un dossier : fichier local nommé `milliers` (ou `ecrasement`) : refusé, « Un dossier porte déjà ce nom… ».
- [ ] Cible apparue entre-temps : choisir un fichier local `vpsctl-t-course2.txt`, et pendant qu'un gros envoi occupe la file, créer `~/vpsctl-t-files/vpsctl-t-course2.txt` sur le VPS. Quand vient son tour : « × ERREUR », « Un élément porte déjà ce nom ». Réessayer : GO / NO GO de remplacement, puis envoi.
- [ ] Dossier sans droit d'écriture : `mkdir ~/vpsctl-t-files/lecture-seule && chmod 555 ~/vpsctl-t-files/lecture-seule`, y envoyer un fichier : erreur « Accès refusé sur le VPS : …/lecture-seule ». Puis `chmod 755`.
- [ ] Espace disque insuffisant : non reproductible sans droits root (quota ou petit système de fichiers) ; couvert par les tests unitaires. Si un VPS de test le permet : message « Espace disque insuffisant sur le VPS (… nécessaires, … libres). ».

## 11. Téléchargement

- [ ] Ctrl+D (ou menu Télécharger, ou l'icône de la ligne) sur `gros-distant.bin` : fenêtre d'enregistrement du système, dossier Téléchargements proposé, nom `gros-distant.bin`.
- [ ] Pendant le téléchargement : le point avance du serveur vers LOCAL, « VPS-PROD-01 → LOCAL · RÉCEPTION ». Dans le dossier local, `.gros-distant.bin.atlas-<8 hex>.part` grossit (afficher les fichiers cachés), pas encore `gros-distant.bin`.
- [ ] Fin : « ✓ REÇU », coche sur le nœud LOCAL, notification « Téléchargé » avec « Afficher ». L'empreinte locale est celle notée sur le VPS. Aucun `.part` restant.
- [ ] « Afficher » (dans la file ou la notification) : l'explorateur du système s'ouvre sur le dossier, le fichier sélectionné (Windows : Explorateur ; Linux : gestionnaire de fichiers, selon le bureau).
- [ ] Remplacement local : télécharger à nouveau vers le même nom : la fenêtre du système demande de confirmer le remplacement. Confirmer : le fichier est remplacé.
- [ ] **Annulation en cours de téléchargement**, vers un fichier local existant (même nom que le précédent, remplacement confirmé) : Annuler à mi-parcours. Le fichier local existant est intact (même empreinte), aucun `.part` restant.
- [ ] Lien vers un fichier (`lien-fichier`) : le téléchargement suit le lien, le fichier local contient « ne pas toucher ».
- [ ] Fichier illisible (`illisible.txt`) : « × ERREUR », « Accès refusé sur le VPS : … ». Aucun fichier local créé.
- [ ] Pas de téléchargement de dossier : ni icône, ni entrée de menu, Ctrl+D sans effet.

## 12. File d'attente

- [ ] Lancer trois envois (plusieurs fichiers d'un coup) puis un téléchargement : un seul « en cours », les autres « ○ EN ATTENTE » avec « Retirer ». En-tête : « 1 en cours · 3 en attente ».
- [ ] Retirer un transfert en attente : il disparaît, les autres continuent.
- [ ] À la fin du transfert actif, le suivant démarre seul.
- [ ] Transfert terminé ou annulé : bouton « Retirer » (croix, infobulle « Retirer … de la file »). Erreur ou annulé : « Réessayer » le remet en attente.
- [ ] Pendant un gros envoi, aller sur Tableau de bord puis revenir : le transfert a continué, la progression est à jour.
- [ ] Ligne en erreur : fond teinté de rouge, croix dans le point de l'orbite figé à la position de la perte, « LOS à … sur … ».
- [ ] **Liaison perdue** pendant un envoi (couper le réseau de la machine locale) : après une vingtaine de secondes (délai des requêtes SFTP), « × ERREUR » avec un message de liaison perdue. Rétablir le réseau (et se reconnecter si l'appli est revenue à l'écran de connexion), ouvrir le dossier : le temporaire `.…atlas-….part`, qui n'a pas pu être supprimé, est signalé comme résidu de transfert (section 14) ; le fichier d'origine éventuel est intact.
- [ ] Couper la liaison (« Couper la liaison ») pendant un transfert : le transfert est annulé et son temporaire supprimé (vérifier sur le VPS). Après reconnexion, la file est vide.
- [ ] Fermeture de l'application (bouton Fermer) pendant un envoi : sur le VPS, aucun temporaire (l'envoi est annulé et son temporaire supprimé à la fermeture).

## 13. Rendu Windows et Linux

- [ ] Windows (WebView2) et Linux (WebKitGTK) : itinéraire, liste, ligne de création, menu, GO / NO GO et file des transferts conformes aux captures (`docs/design/ecran/fichiers/`, `transfert.png`, `menu-contextuel.png`, `go-no-go.png`).
- [ ] L'orbite : le point glisse en douceur sous WebKitGTK (transition de `transform` sur un élément SVG).
- [ ] Défilement de `milliers` fluide sous WebKitGTK.
- [ ] **Presse-papiers sous WebKitGTK** : Copier le chemin (menu et Ctrl+Maj+C) puis coller dans un autre programme. Si la notification « Copie impossible » apparaît, ou si rien n'est collé, le noter : il faudra le plugin clipboard-manager.
- [ ] Fenêtres de sélection natives sous Linux (GTK ou portail) : ouverture multiple, enregistrement avec confirmation de remplacement.
- [ ] Mouvement réduit activé : le menu, le fondu de la liste et le point de l'orbite n'ont plus d'animation visible.

## 14. Résidus de transfert

- [ ] Dans `vpsctl-t-files`, les trois résidus simulés portent la mention « RÉSIDU DE TRANSFERT » (étiquette ambre).
- [ ] Infobulle de `.rapport.pdf.atlas-0123abcd.part` : fichier temporaire d'un envoi interrompu vers « rapport.pdf », contenu incomplet, peut être supprimé.
- [ ] Infobulle de `.config.yml.atlas-deadbeef.old` (`config.yml` existe) : ancien contenu mis de côté, vérifier `config.yml` puis supprimer le résidu.
- [ ] Infobulle de `.notes.txt.atlas-89abcdef.old` (`notes.txt` n'existe pas) : « c'est probablement la seule copie du fichier d'origine », avec la marche à suivre. Le GO / NO GO de sa suppression le rappelle aussi.
- [ ] Récupération : F2 sur `.notes.txt.atlas-89abcdef.old`, renommer en `notes.txt` : le fichier est récupéré (`cat` sur le VPS), la mention disparaît.
- [ ] Un dossier nommé comme un résidu (`mkdir ~/vpsctl-t-files/.x.atlas-00000000.part`) n'est pas signalé.

### Récupération manuelle après un écrasement interrompu

Un écrasement se fait en trois étapes : la cible `<nom>` est renommée en `.<nom>.atlas-<hex>.old`, le temporaire complet `.<nom>.atlas-<hex>.part` prend sa place, puis le `.old` est supprimé. Si la liaison tombe pendant ces étapes, le contenu complet existe toujours sous l'un de ces noms (le même `<hex>` relie les deux temporaires d'un écrasement) :

| Ce que contient le dossier | Signification | Récupération |
|---|---|---|
| `.part` seul (et `<nom>` éventuel) | Envoi interrompu avant la fin ; `<nom>` n'a pas été touché | Supprimer le `.part` |
| `.old` et `.part`, sans `<nom>` | Coupure entre les étapes 1 et 2 : `.old` est l'original, `.part` le nouveau contenu, complet | Renommer le `.old` en `<nom>` pour revenir à l'original (ou le `.part` en `<nom>` pour terminer l'envoi), puis supprimer l'autre |
| `.old` seul, sans `<nom>` | Cas précédent dont le `.part` a été supprimé : le `.old` est la **seule copie** de l'original | Renommer le `.old` en `<nom>` |
| `.old` et `<nom>` | Coupure entre les étapes 2 et 3 : l'envoi a abouti, le `.old` est l'ancienne version | Vérifier `<nom>`, puis supprimer le `.old` |

Depuis l'appli : F2 sur le résidu, saisir `<nom>`, Entrée. Depuis un terminal : `mv '.<nom>.atlas-<hex>.old' '<nom>'` (les droits d'origine sont ceux du `.old`).

Un téléchargement interrompu par l'arrêt brutal de l'appli peut laisser un `.<nom>.atlas-<hex>.part` dans le dossier local : il n'est jamais le seul exemplaire (l'original est sur le VPS), le supprimer.

## 15. Sécurité (contrôle rapide)

Couvert par les tests unitaires (`cargo test`). Contrôle facultatif depuis les outils de développement de la fenêtre (console JavaScript) :

```js
const invoke = window.__TAURI_INTERNALS__.invoke
await invoke("list_dir", { path: "/home/../root" })
await invoke("create_dir", { path: "/tmp/a\u0000b" })
await invoke("create_file", { path: "relatif.txt" })
await invoke("rename", { fromPath: "/tmp/x", toPath: "/tmp/.." })
await invoke("delete_path", { path: "/", recursive: true })
await invoke("delete_path", { path: "/home", recursive: false })
await invoke("rename", { fromPath: await invoke("home_dir"), toPath: "/tmp/vpsctl-t-home" })
```

- [ ] Chaque appel est refusé avec `InvalidInput` : chemin invalide, racine, dossier qui contient le dossier personnel, dossier personnel. Rien n'est créé, renommé ni supprimé sur le VPS.
- [ ] Pendant un transfert, sur le VPS, `ps -u "$USER" -f` ne liste, pour la session de l'appli, que le serveur SFTP (`sftp-server`) : aucune commande shell n'est lancée par l'explorateur.

## 16. Nettoyage

```sh
chmod -R u+rwx ~/vpsctl-t-files 2>/dev/null
rm -rf ~/vpsctl-t-files ~/vpsctl-t-cible

# Vérification
ls -d ~/vpsctl-t-* 2>/dev/null
ls -la ~/.ssh      # inchangé
```

Local :

```powershell
Remove-Item -Recurse -Force "$env:USERPROFILE\vpsctl-t-local"
Remove-Item "$env:USERPROFILE\Downloads\gros-distant.bin", "$env:USERPROFILE\Downloads\vers-fichier" -ErrorAction SilentlyContinue
Get-ChildItem "$env:USERPROFILE\Downloads" -Force -Filter '.*atlas-*'
```

```sh
rm -rf ~/vpsctl-t-local
rm -f ~/Téléchargements/gros-distant.bin ~/Downloads/gros-distant.bin
ls -A ~/Téléchargements ~/Downloads 2>/dev/null | grep 'atlas-'
```

- [ ] Après le nettoyage, plus aucun objet `vpsctl-t-` sur le VPS ni en local, aucun temporaire `.…atlas-…` restant, et `~/.ssh` est inchangé.
