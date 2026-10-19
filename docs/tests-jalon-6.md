# Tests manuels du jalon 6 : création et suppression des services systemd

À dérouler sur Windows puis sur Linux. Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure.

Tous les services de test sont nommés `vpsctl-t-*` et les fichiers annexes sont dans `~/vpsctl-t-*` ; tout est supprimé à la fin (section 10).

## Préparation

- Lancer l'application : `npm run tauri dev`, se connecter au VPS.
- Garder un terminal SSH ouvert sur le VPS, avec le même compte, pour les vérifications côté serveur.
- Noter l'état du lingering : `loginctl show-user "$USER" --property=Linger` doit afficher `Linger=yes` (voir `docs/02-preparation-vps.md`).
- Repère : `U=~/.config/systemd/user` dans le terminal SSH.

## 1. Écran de création

- [y] Tableau de bord : l'en-tête de « Services systemd » a un bouton « Nouveau service ».
- [y] Clic : écran « I · NOUVELLE UNITÉ / Créer un service systemd », `~/.config/systemd/user/` à droite du titre ; la navigation surligne **Tableau de bord**.
- [y] Mise en page conforme aux captures (`docs/design/ecran/systemd-create/`) :
  - panneau « Identité et exécution » (nom avec suffixe `.service` accolé, description, commande, dossier de travail pré-rempli avec `~`) ;
  - panneau « Politique de redémarrage » : trois cartes `no`, `on-failure` (choisie par défaut, bordée d'ambre), `always`, chacune avec sa description ; `RestartSec=5` à droite du bandeau ;
  - panneau « Variables d'environnement » avec « Ajouter » ;
  - Annuler et « Créer le service » sous les panneaux, à droite ;
  - aperçu à droite, « APERÇU · nom.service ».
- [y] Les cartes de redémarrage se choisissent à la souris et au clavier (Tab jusqu'au groupe, puis flèches).
- [y] Lingering actif : aucun avertissement.
- [y] Annuler : retour au tableau de bord, rien n'est créé.
- [y] Fenêtre étroite (navigation réduite aux numéraux) : le numéral I reste surligné, formulaire et aperçu restent lisibles.

## 2. Validation instantanée et aperçu

Saisir chaque valeur et vérifier le message sous le champ dès la saisie. Le bouton « Créer le service » refuse l'envoi tant qu'un problème subsiste.

- [y] Nom `Bot` ou `bot_1` : « Minuscules, chiffres et tirets, sans tiret au début. » ; `-bot` : idem ; `bot.service` : « Saisissez le nom sans « .service ». » ; 65 caractères : « 64 caractères au plus. »
- [y] Commande vide : « La commande est requise. »
- [y] Commande `-/bin/true`, `@/bin/true`, `|echo x`, `"-/bin/true"` : « Le préfixe spécial « … » de systemd n'est pas pris en charge… »
- [y] Commande `~/bin/x` : « systemd ne développe pas ~… » ; `bin/x` ou `./x` : « Le programme est un chemin absolu ou un simple nom. »
- [y] Commande `/bin/echo a ; /bin/rm x` : message sur le `;` isolé. `/bin/echo a\; b` et `/bin/sh -c 'a ; b'` sont acceptés.
- [y] Commande `/bin/echo 'a` : « Guillemet non fermé. » ; `/bin/echo a \` : « Ne doit pas se terminer par « \ ». »
- [y] Description `C:\` : « Ne doit pas se terminer par « \ ». »
- [y] Dossier `srv/app`, `-/srv`, `~user/x`, `/srv/../etc`, vide : message sous le champ.
- [y] Variable : clé `1A` ou `A-B` : « Lettres, chiffres et _, sans chiffre au début. » ; deux fois la même clé : « Clé en double. » ; valeur sans clé : « Clé requise. » ; ligne entièrement vide : ignorée, sans message.
- [y] Coller dans la commande un texte sur deux lignes : le champ n'a qu'une ligne (le retour à la ligne disparaît ou le message « Retour à la ligne… » s'affiche) ; dans tous les cas, rien n'atteint le fichier.
- [y] Aperçu avant toute saisie valide : gabarit avec « … », sans étiquette LIVE, et le problème en cours sous le gabarit.
- [y] Saisie valide : environ 300 ms après la dernière frappe, le texte exact du fichier s'affiche avec l'étiquette LIVE. Taper vite : l'aperçu ne clignote pas à chaque touche.
- [y] Rendre un champ invalide : le dernier aperçu valide reste affiché, en retrait, sans LIVE, avec le problème en dessous.

## 3. Service qui fonctionne

Créer `vpsctl-t-ok` :

| Champ | Valeur |
|---|---|
| Nom | `vpsctl-t-ok` |
| Description | `Test vpsctl` |
| Commande | `/bin/sh -c 'while true; do echo tic; sleep 5; done'` |
| Dossier de travail | `~` |
| Redémarrage | `on-failure` |

- [y] « Créer le service » : chrono « Création T+… » sur le bouton, puis retour au tableau de bord. La ligne `vpsctl-t-ok.service` apparaît aussitôt « En transition » (« Démarrage » et chrono), puis environ 2,5 s plus tard « Nominal », avec l'animation d'allumage et la notification « Service créé et démarré ».
- [y] Sur le VPS, `systemctl --user cat vpsctl-t-ok.service` : après la ligne de commentaire `# /home/<compte>/.config/systemd/user/vpsctl-t-ok.service`, le texte est **identique** à l'aperçu affiché avant la création.
- [y] `systemctl --user is-enabled vpsctl-t-ok` : `enabled` ; `ls -l $U/default.target.wants/` contient le lien `vpsctl-t-ok.service`.
- [y] `` : `644`. `ls -a $U` : aucun fichier `.vpsctl-t-ok.service.atlas-*.part`.
- [y] Logs (bouton Télémétrie de la ligne) : une ligne « tic » toutes les 5 s.
- [y] Si `~/.config/systemd/user` n'existait pas avant (compte neuf), il a été créé, ainsi que ses parents.

## 4. %, guillemets et variable avec caractères spéciaux

Créer `vpsctl-t-escape` :

| Champ | Valeur |
|---|---|
| Nom | `vpsctl-t-escape` |
| Description | `Test 100% "échappé"` |
| Commande | `/bin/sh -c 'printf "%s\n" "annee=$(date +%Y)" "pct=100%" "var=$VPSCTL_T_VAR" "home=$HOME"; exec sleep infinity'` |
| Dossier de travail | `~/vpsctl-t-dir` (créer le dossier avant : `mkdir ~/vpsctl-t-dir`) |
| Redémarrage | `no` |
| Variable | `VPSCTL_T_VAR` = `a "b" \c %h $USER` |

- [ ] L'aperçu, puis `systemctl --user cat vpsctl-t-escape.service`, contiennent exactement :

  ```ini
  Description=Test 100%% "échappé"
  WorkingDirectory=%h/vpsctl-t-dir
  ExecStart=/bin/sh -c 'printf "%%s\n" "annee=$$(date +%%Y)" "pct=100%%" "var=$$VPSCTL_T_VAR" "home=$$HOME"; exec sleep infinity'
  Restart=no
  Environment="VPSCTL_T_VAR=a \"b\" \\c %%h $USER"
  ```

- [y] Le service passe « Nominal ». Dans le tableau de bord, la description affichée est `Test 100% "échappé"`.
- [y] Logs : exactement quatre lignes, avec l'année en cours et le dossier personnel :

  ```
  annee=2026
  pct=100%
  var=a "b" \c %h $USER
  home=/home/<compte>
  ```

  (`%h` et `$USER` arrivent tels quels : aucune substitution par systemd.)
- [y] `systemctl --user show vpsctl-t-escape -p WorkingDirectory --value` : `/home/<compte>/vpsctl-t-dir`.

## 5. Service qui échoue au démarrage

Créer `vpsctl-t-echec` : commande `/bin/sh -c 'echo "démarrage impossible" >&2; exit 3'`, redémarrage `no`.

- [y] Retour au tableau de bord, « En transition », puis « En échec » avec la notification d'erreur persistante « Échec du démarrage · vpsctl-t-echec.service · failed/failed » et « Voir la télémétrie ».
- [y] « Voir la télémétrie » : la ligne « démarrage impossible » et les messages de systemd (`status=3/NOTIMPLEMENTED`…).
- [y] Le service reste créé : `ls $U/vpsctl-t-echec.service` existe, `systemctl --user is-enabled vpsctl-t-echec` : `enabled`.
- [y] Variante : `vpsctl-t-absent`, commande `/usr/bin/vpsctl-t-introuvable`. Même verdict « En échec » ; les logs indiquent que l'exécutable est introuvable (`203/EXEC`).

## 6. Nom déjà pris

- [y] Saisir `vpsctl-t-ok` dans le formulaire : « Un service porte déjà ce nom : vpsctl-t-ok.service. » dès la saisie, création refusée.
- [y] Contrôle du backend, même nom, depuis la console de développement (clic droit → Inspecter, en mode développement) :

  ```js
  await window.__TAURI_INTERNALS__.invoke("service_create", { def: { name: "vpsctl-t-ok", description: "", execStart: "/bin/true", workingDir: "~", restart: "no", env: [] } })
  ```

  Erreur `AlreadyExists` avec le chemin du fichier ; `systemctl --user cat vpsctl-t-ok` montre le texte d'origine (« tic »), intact.
- [y] Unité du même nom ailleurs : sur le VPS, `mkdir -p ~/.local/share/systemd/user && printf '[Service]\nExecStart=/bin/true\n' > ~/.local/share/systemd/user/vpsctl-t-ailleurs.service && systemctl --user daemon-reload`. Créer `vpsctl-t-ailleurs` dans l'application : message « Un élément porte déjà ce nom : /home/<compte>/.local/share/systemd/user/vpsctl-t-ailleurs.service », rien n'est écrit dans `$U`.
- [y] Dossier de surcharge orphelin : `mkdir $U/vpsctl-t-reste.service.d`. Créer `vpsctl-t-reste` : « Un dossier porte déjà ce nom : …/vpsctl-t-reste.service.d », aucun fichier `vpsctl-t-reste.service` créé.

## 7. Suppression

### Cas où Supprimer est désactivé

Sur le VPS :

```sh
# Gabarit et instance
printf '[Service]\nExecStart=/bin/sleep infinity\n' > $U/vpsctl-t-gab@.service
# Unité masquée (lien vers /dev/null)
systemctl --user mask vpsctl-t-masque.service
# Unité liée (fichier situé ailleurs)
mkdir -p ~/vpsctl-t-units
printf '[Service]\nExecStart=/bin/sleep infinity\n' > ~/vpsctl-t-units/vpsctl-t-lie.service
systemctl --user link ~/vpsctl-t-units/vpsctl-t-lie.service
systemctl --user daemon-reload
systemctl --user start vpsctl-t-gab@1.service
```

- [y] `vpsctl-t-gab@1.service` : corbeille grisée, infobulle « instance d'un gabarit : elle n'a pas de fichier propre ».
- [y] `vpsctl-t-masque.service` : corbeille grisée, « unité masquée : démasquez-la avant de la supprimer ».
- [y] `vpsctl-t-lie.service` : corbeille grisée, « lien vers un fichier situé ailleurs (systemctl link) ».
- [y] Dans chaque ligne, la corbeille est séparée des autres actions par un trait vertical, comme pour les conteneurs.

### Service avec dossier de surcharge

Sur le VPS :

```sh
mkdir -p $U/vpsctl-t-ok.service.d
printf '[Service]\nEnvironment=VPSCTL_T_SURCHARGE=1\n' > $U/vpsctl-t-ok.service.d/override.conf
systemctl --user daemon-reload
```

- [ ] Supprimer `vpsctl-t-ok.service` : fenêtre GO / NO GO « Supprimer le service ? », sujet `vpsctl-t-ok.service`, mention « Irréversible », focus sur NO GO, avec dans les conséquences :
  - l'arrêt du service (il tourne) ;
  - la désactivation ;
  - « Fichier d'unité supprimé » et le chemin complet `/home/<compte>/.config/systemd/user/vpsctl-t-ok.service` ;
  - « Dossier de surcharge supprimé, avec son élément » et son chemin ;
  - le rechargement de systemd.
- [y] NO GO (ou Échap) : rien ne change.
- [y] GO : la ligne passe « En transition » (« Suppression » et chrono), puis disparaît ; notification « Service supprimé ».
- [y] Vérification sur le VPS, il ne reste rien :

  ```sh
  systemctl --user list-unit-files 'vpsctl-t-ok*'      # « 0 unit files listed. »
  ls -a $U/ $U/default.target.wants/ | grep vpsctl-t-ok # aucune ligne
  systemctl --user status vpsctl-t-ok                  # « Unit vpsctl-t-ok.service could not be found. »
  pgrep -u "$USER" -af 'echo tic'                      # aucune ligne
  ```

### Autres suppressions

- [y] `vpsctl-t-echec.service` (en échec, sans dossier de surcharge) : la fenêtre indique « Service arrêté » en condition vérifiée et pas d'arrêt dans les conséquences. Après GO, `systemctl --user list-units --all --failed | grep vpsctl-t-echec` ne renvoie rien (`reset-failed` effectué).
- [y] Service qui redémarre en boucle : créer `vpsctl-t-boucle` (commande `/bin/false`, redémarrage `always`). Sa ligne alterne entre « En transition » et « En échec » ; Supprimer reste disponible et la suppression aboutit.
- [y] Échec en cours de séquence : recréer `vpsctl-t-ok` (section 3), puis `mkdir $U/vpsctl-t-ok.service.d && touch $U/vpsctl-t-ok.service.d/a.conf && chmod 500 $U/vpsctl-t-ok.service.d`. Supprimer → GO : notification d'erreur persistante « Échec de la suppression », « Arrêt à l'étape « suppression du dossier de surcharge » : Accès refusé sur le VPS : … Service arrêté et désactivé, fichier en place, dossier de surcharge peut-être partiellement supprimé. » La ligne reste, « Arrêté ».
- [y] `chmod 700 $U/vpsctl-t-ok.service.d`, puis Supprimer à nouveau : la suppression aboutit.

## 8. Survie après la fin des sessions (lingering)

- [y] Créer `vpsctl-t-linger` : commande `/bin/sh -c 'while true; do date +%T; sleep 10; done'`. « Nominal ».
- [y] Couper la liaison dans l'application et fermer **toutes** les sessions SSH du compte. Depuis une autre machine ou un autre compte, si possible, `loginctl list-sessions` ne montre plus de session pour ce compte.
- [y] Attendre au moins 2 minutes, se reconnecter : `vpsctl-t-linger.service` est toujours « Nominal » ; ses logs montrent une heure toutes les 10 s **sans interruption** pendant l'absence ; `systemctl --user show vpsctl-t-linger -p ActiveEnterTimestamp` est antérieur à la déconnexion.
- [y] Facultatif, si le compte peut modifier son lingering : `loginctl disable-linger`, puis ouvrir l'écran de création : avertissement « Lingering inactif pour ce compte… » avec `loginctl enable-linger`. Rétablir avec `loginctl enable-linger` et vérifier `Linger=yes`.

## 9. Entrées hostiles (console de développement)

```js
const def = { name: "vpsctl-t-h", description: "", execStart: "/bin/true", workingDir: "~", restart: "no", env: [] };
await window.__TAURI_INTERNALS__.invoke("service_create", { def: { ...def, name: "x; rm -rf ~" } })
await window.__TAURI_INTERNALS__.invoke("service_create", { def: { ...def, execStart: "/bin/true\nExecStartPre=/usr/bin/touch /tmp/vpsctl-pwned" } })
await window.__TAURI_INTERNALS__.invoke("service_create", { def: { ...def, description: "x\rExecStartPre=/usr/bin/touch /tmp/vpsctl-pwned" } })
await window.__TAURI_INTERNALS__.invoke("service_create", { def: { ...def, workingDir: "-/tmp" } })
await window.__TAURI_INTERNALS__.invoke("service_create", { def: { ...def, env: [{ key: "A=B", value: "1" }] } })
await window.__TAURI_INTERNALS__.invoke("service_unit_preview", { def: { ...def, env: [{ key: "A", value: "x\"\nExecStartPre=/usr/bin/touch /tmp/vpsctl-pwned" }] } })
await window.__TAURI_INTERNALS__.invoke("service_delete", { unit: "x; touch /tmp/vpsctl-pwned.service" })
await window.__TAURI_INTERNALS__.invoke("service_delete", { unit: "dbus.service" })
```

- [y] Chaque appel est refusé avec `InvalidInput`, dont le message nomme le champ. Sur le VPS, `ls /tmp/vpsctl-pwned` : fichier absent ; `ls $U | grep vpsctl-t-h` : rien.

## 10. Nettoyage

Dans l'application : supprimer les services `vpsctl-t-*` encore listés et supprimables (`vpsctl-t-escape`, `vpsctl-t-absent`, `vpsctl-t-linger`, `vpsctl-t-boucle`…).

Sur le VPS :

```sh
systemctl --user stop 'vpsctl-t-gab@*' vpsctl-t-lie.service
systemctl --user disable vpsctl-t-lie.service
systemctl --user unmask vpsctl-t-masque.service
rm -f $U/vpsctl-t-gab@.service
rm -rf $U/vpsctl-t-reste.service.d ~/.local/share/systemd/user/vpsctl-t-ailleurs.service
rm -rf ~/vpsctl-t-units ~/vpsctl-t-dir
systemctl --user daemon-reload
systemctl --user reset-failed

# Vérification : plus aucune trace
systemctl --user list-unit-files 'vpsctl-t-*'
systemctl --user list-units --all 'vpsctl-t-*'
ls -a $U $U/default.target.wants 2>/dev/null | grep vpsctl-t
```

- [y] Les deux `systemctl` n'affichent aucune unité, le `ls` aucune ligne, et la liste des services de l'application est revenue à l'état de départ.
