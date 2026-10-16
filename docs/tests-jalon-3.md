# Tests manuels du jalon 3 : piloter les services et logs en direct

À dérouler sur Windows puis sur Linux. Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure.

Tous les objets de test sont préfixés `vpsctl-t-` pour être nettoyés facilement (section 10).

## Préparation

- Lancer l'application : `npm run tauri dev`, se connecter au VPS, ouvrir le tableau de bord.
- Garder un terminal SSH ouvert sur le VPS, avec le même compte, pour les vérifications côté serveur.
- Noter l'état de départ, pour comparer au nettoyage :

```sh
docker ps -a --format '{{.Names}}\t{{.State}}\t{{.Status}}'
docker volume ls
systemctl --user list-units --type=service --all
ls -la ~/.config/systemd/user/
```

### Cas de test Docker

```sh
# Écrit en alternance sur la sortie standard et la sortie d'erreur, une paire par seconde
docker run -d --name vpsctl-t-logs alpine sh -c 'i=0; while :; do i=$((i+1)); echo "out $i"; echo "err $i" >&2; sleep 1; done'

# Silencieux : n'écrit jamais rien (arrêt des flux, section 7)
docker run -d --name vpsctl-t-quiet alpine sleep 1d

# En marche, pour Arrêter et Redémarrer. sleep est le PID 1 et ignore SIGTERM :
# docker stop attend 10 s avant SIGKILL (chrono, délai de 45 s non atteint)
docker run -d --name vpsctl-t-run alpine sleep 1d

# Créé, jamais démarré : démarre puis échoue après 1 s (vérification différée)
docker create --name vpsctl-t-crash alpine sh -c 'echo démarrage; sleep 1; echo plantage >&2; exit 1'

# À supprimer : sorti, avec un volume nommé et un volume anonyme (conservés)
docker run --name vpsctl-t-rm -v vpsctl-t-vol:/data -v /anon alpine true
docker inspect -f '{{range .Mounts}}{{.Name}} {{end}}' vpsctl-t-rm   # noter les deux noms de volume
```

### Cas de test systemd

```sh
mkdir -p ~/.config/systemd/user

# Messages de différentes priorités, toutes les 2 s
cat > ~/.config/systemd/user/vpsctl-t-prio.service <<'EOF'
[Unit]
Description=vpsctl test : priorités du journal

[Service]
ExecStart=/bin/sh -c 'while :; do echo "<3>erreur, priorité 3"; echo "<4>avertissement, priorité 4"; echo "<6>information, priorité 6"; echo "sans préfixe"; echo "sortie d erreur sans préfixe" >&2; printf "latin-1 : caf\\351\\n"; sleep 2; done'
EOF

# Échoue 1 s après son démarrage (type simple : le démarrage « réussit »)
cat > ~/.config/systemd/user/vpsctl-t-crash.service <<'EOF'
[Unit]
Description=vpsctl test : échec juste après le démarrage

[Service]
ExecStart=/bin/sh -c 'echo démarrage; sleep 1; echo "<3>plantage"; exit 1'
EOF

# Lent à démarrer : « activating » pendant 8 s (--no-block, transition sans verdict)
cat > ~/.config/systemd/user/vpsctl-t-slow.service <<'EOF'
[Unit]
Description=vpsctl test : démarrage lent

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/bin/sleep 8
EOF

# Service ordinaire, pour Arrêter et Redémarrer
cat > ~/.config/systemd/user/vpsctl-t-svc.service <<'EOF'
[Unit]
Description=vpsctl test : service ordinaire

[Service]
ExecStart=/bin/sh -c 'while :; do echo tick; sleep 5; done'
EOF

# Nom hors du format pilotable
cat > ~/.config/systemd/user/'vpsctl-t\x2descaped.service' <<'EOF'
[Unit]
Description=vpsctl test : nom échappé

[Service]
ExecStart=/bin/sleep infinity
EOF

systemctl --user daemon-reload
systemctl --user start vpsctl-t-prio.service vpsctl-t-svc.service
systemctl --user mask vpsctl-t-masked.service
```

### Priorité donnée par le préfixe `<N>`

Par défaut, `SyslogLevelPrefix=` vaut `true` pour un service dont la sortie va au journal, ce qui est le cas par défaut (`systemd.exec(5)`) : une ligne commençant par `<3>` est enregistrée en priorité 3, préfixe retiré. À vérifier sur le VPS :

```sh
journalctl --user -u vpsctl-t-prio.service -n 6 -o json --output-fields=MESSAGE,PRIORITY --no-pager
```

- [ ] `erreur, priorité 3` a `"PRIORITY":"3"`, `avertissement, priorité 4` a `"4"`, `information, priorité 6` a `"6"`, sans le préfixe dans `MESSAGE`.
- [ ] `sans préfixe` et `sortie d erreur sans préfixe` ont `"6"` : journald ne distingue pas la sortie d'erreur (elle partage le flux de la sortie standard).
- [ ] La ligne `latin-1 : caf\351` a un `MESSAGE` en tableau d'octets (`[108,97,…,233]`).

Si le préfixe n'était pas interprété : vérifier que l'unité ne contient ni `SyslogLevelPrefix=no` ni `StandardOutput=` autre que `journal`. Méthode de repli : `systemd-cat -p err echo …` dans `ExecStart`.

## 1. Actions sur les conteneurs

- [ ] Chaque ligne a une colonne Actions : Démarrer, Arrêter, Redémarrer, Logs, puis, isolée par un séparateur, Supprimer (icône corbeille rouge).
- [ ] Chaque icône a une infobulle au survol **et** au focus clavier (Tab). Une action indisponible est grisée et son infobulle en donne la raison (« Démarrer vpsctl-t-run : déjà en marche »). Elle reste focalisable mais un clic n'a aucun effet.
- [ ] Lecteur d'écran (Narrateur ou Orca), ou outils de développement : chaque bouton a un `aria-label` avec l'action et le nom du conteneur.
- [ ] `vpsctl-t-run` (en marche) : Démarrer et Supprimer indisponibles ; Arrêter, Redémarrer, Logs disponibles.
- [ ] `vpsctl-t-rm` (sorti) : Arrêter indisponible ; Démarrer, Redémarrer, Logs, Supprimer disponibles.
- [ ] **Démarrer** `vpsctl-t-crash` : aucune confirmation. La ligne passe aussitôt en « Démarrage · T+00 » (demi-disque), le bouton Démarrer affiche son chrono, les autres actions sauf Logs sont indisponibles.
- [ ] **Arrêter** `vpsctl-t-run` : fenêtre GO / NO GO « Arrêter le conteneur ? » avec ses trois conséquences. Le focus est sur NO GO ; Échap ferme sans rien faire. GO : « Arrêt · T+… » pendant environ 10 s, sans erreur de délai. Puis « Arrêté · code 137 » et la notification « Conteneur arrêté · vpsctl-t-run · 10,… s ».
- [ ] **Redémarrer** `vpsctl-t-run` (le redémarrer d'abord s'il est arrêté) : GO / NO GO « Redémarrer le conteneur ? », puis « Redémarrage · T+… » environ 13 s (arrêt de 10 s, puis relecture à 2,5 s), puis « Nominal » et « Conteneur redémarré ».
- [ ] **Supprimer** `vpsctl-t-rm` : GO / NO GO « Supprimer le conteneur ? », mention « Irréversible », coche « Conteneur arrêté » suivie du statut brut (`Exited (0) …`), puis « Les volumes sont conservés. » et « L'image est conservée : alpine. ». GO est rouge et à droite.
- [ ] Après GO : la ligne disparaît, notification « Conteneur supprimé ». Sur le VPS, `docker volume ls` liste toujours `vpsctl-t-vol` **et** le volume anonyme noté en préparation ; `docker image ls alpine` liste toujours l'image.
- [ ] Supprimer un conteneur en marche est impossible depuis l'appli (bouton indisponible). Côté serveur, `docker rm` sans `-f` le refuserait de toute façon.

## 2. Actions sur les services

- [ ] Colonne Actions : Démarrer, Arrêter, Redémarrer, Logs. Pas de suppression (jalon 6).
- [ ] `vpsctl-t-masked.service` (« masqué ») et `vpsctl-t\x2descaped.service` (« non pilotable ») n'ont **que** le bouton Logs. Pour l'unité masquée, Logs fonctionne ; pour le nom échappé, Logs est indisponible (« nom hors du format accepté »).
- [ ] **Arrêter** `vpsctl-t-svc.service` : GO / NO GO « Arrêter le service ? » avec ses deux conséquences ; GO : « Arrêt · T+… » environ 2,5 s, puis « Arrêté » et « Service arrêté ».
- [ ] **Redémarrer** `vpsctl-t-svc.service` : GO / NO GO, puis « Nominal » et « Service redémarré ».
- [ ] **Démarrer** `vpsctl-t-slow.service` : direct. La commande rend la main tout de suite (`--no-block`) : « Démarrage · T+… » pendant 2,5 s, puis la ligne reste « En transition » (état réel `activating`), **sans notification**. Elle passe « Nominal » à un rafraîchissement suivant, environ 8 s après le clic.
- [ ] Sur le VPS, pendant ce temps, `systemctl --user is-active vpsctl-t-slow.service` répond `activating`.

## 3. Vérification différée

- [ ] **Docker** : après Démarrer sur `vpsctl-t-crash`, la ligne reste « Démarrage · T+… » environ 2,5 s, même si un rafraîchissement automatique passe entre-temps (le noter avec l'onglet Réseau : un `docker_list` pendant la transition ne change pas la ligne).
- [ ] Puis « En échec · code 1 » et une notification d'**erreur** « Échec du démarrage · vpsctl-t-crash · code 1 », qui reste affichée tant qu'on ne la ferme pas.
- [ ] Le lien « Voir la télémétrie » (avec une flèche dessinée, pas un caractère) ouvre la console ancrée sur `vpsctl-t-crash` : on y voit `démarrage` en `out` puis `plantage` en `err`. Le conteneur étant déjà arrêté, `docker logs -f` se termine aussitôt : bannière « LOS · Fin du flux · code 0 » (code de `docker logs`, pas du conteneur). La notification se ferme.
- [ ] **systemd** : même test avec Démarrer sur `vpsctl-t-crash.service` : « Échec du démarrage · vpsctl-t-crash.service · failed/failed », et dans la console `plantage` en **ERR rouge** (priorité 3). `journalctl -f` continue de suivre le journal : pas de bannière LOS.
- [ ] Erreur de commande (facultatif) : sur le VPS, `docker rm -f vpsctl-t-run` puis, sans attendre le rafraîchissement, cliquer Arrêter sur sa ligne et GO. Notification d'erreur « Échec de l'arrêt » avec le message de Docker (« No such container »), et la ligne disparaît au rafraîchissement. Recréer ensuite `vpsctl-t-run`.

## 4. Animation de changement d'état

- [ ] Après une action vérifiée qui change l'état (Démarrer `vpsctl-t-svc.service` arrêté, Arrêter `vpsctl-t-run`) : la forme de l'état joue **une fois** une courte trajectoire (arrivée en arc pour l'allumage, contraction pour l'extinction), en environ 300 ms.
- [ ] Changement fait depuis le VPS (`docker stop -t 2 vpsctl-t-run`, `systemctl --user stop vpsctl-t-svc.service`) : la ligne change au rafraîchissement automatique **sans** animation.
- [ ] Les rafraîchissements suivants ne rejouent pas l'animation.
- [ ] Avec le mouvement réduit activé (Windows : Paramètres → Accessibilité → Effets visuels → Effets d'animation désactivés ; GNOME : Accessibilité → Réduire les animations), l'état change sans animation visible.

## 5. Console ancrée

- [ ] Logs sur `vpsctl-t-logs` : un panneau s'ouvre en bas du tableau de bord. La zone des tableaux **rétrécit** au-dessus et défile seule ; le panneau ne recouvre aucune ligne (faire défiler jusqu'au bas des services pour le vérifier).
- [ ] En-tête : « CONTENEUR vpsctl-t-logs docker logs -f vpsctl-t-logs », interrupteur « Défilement auto », Effacer, Interrompre le flux, Fermer.
- [ ] Pendant l'ouverture, « Acquisition du flux · T+… » s'affiche brièvement (pas de spinner). Puis les 200 dernières lignes, et une paire `out` / `err` par seconde, avec l'heure locale à la milliseconde.
- [ ] `err` est neutre : même couleur de texte que `out`, sans fond rouge. Légende : « out sortie standard », « err sortie d'erreur, sans gravité implicite ». Compteur « N lignes · M sur stderr ».
- [ ] Logs sur `vpsctl-t-prio.service` : la console remplace la précédente. En-tête « SERVICE … journalctl --user -fu vpsctl-t-prio.service ». `erreur, priorité 3` en **ERR** rouge sur fond teinté ; `avertissement, priorité 4` en `wrn` ; les autres en `inf`. La ligne latin-1 s'affiche `latin-1 : caf` suivi du caractère de remplacement. Légende « inf · wrn · ERR priorité 0 à 3 » ; compteur « N lignes · M ERR ».
- [ ] Remonter dans l'historique (molette) : l'interrupteur « Défilement auto » se désactive, la vue ne bouge plus, et un bouton « N nouvelles lignes » (avec une flèche dessinée) apparaît et compte les lignes. Un clic revient en bas et réactive le défilement.
- [ ] Désactiver puis réactiver l'interrupteur à la main : retour en bas.
- [ ] Effacer : la console se vide, le compteur repasse à 0, les nouvelles lignes continuent d'arriver.
- [ ] Interrompre le flux : bannière « LOS · Arrêt propre · HH:MM:SS » ; le bouton devient « Rejouer », qui rouvre le flux avec l'historique.
- [ ] Fin d'elle-même : flux ouvert sur `vpsctl-t-logs`, puis sur le VPS `docker stop -t 1 vpsctl-t-logs`. Bannière « LOS · Fin du flux · code 0 » (ou un autre code). Redémarrer le conteneur depuis l'appli pour la suite.
- [ ] Mémoire bornée (facultatif, charge le VPS quelques secondes) : `docker run -d --name vpsctl-t-loud alpine sh -c 'while :; do echo ligne; done'`, ouvrir ses logs pendant 10 s, puis l'arrêter (`docker rm -f vpsctl-t-loud`). Le compteur plafonne à « 5000 lignes », l'interface reste fluide (défilement, boutons), et des avis « N lignes non affichées : débit trop élevé » apparaissent en texte discret.
- [ ] Défilement horizontal : une ligne plus large que le panneau se lit en faisant défiler horizontalement (pas de retour à la ligne).

## 6. Notifications

- [ ] Succès : cercle avec coche, « SUCCÈS », titre, nom et durée ; fermée seule après 5 s.
- [ ] Erreur : carré avec croix, bordure rouge, « ERREUR » ; reste jusqu'au clic sur la croix (« Fermer la notification »).
- [ ] Enchaîner quatre actions rapides (par exemple Démarrer puis Arrêter deux services) : jamais plus de trois notifications à l'écran, la plus ancienne disparaît.
- [ ] Console ancrée ouverte, fenêtre réduite en hauteur (environ 600 px) : les notifications, en haut à droite, ne recouvrent jamais le panneau.

## 7. Aucun processus laissé sur le serveur

Commande de vérification, sur le VPS (elle ne se liste pas elle-même) :

```sh
pgrep -u "$USER" -af 'docker logs|journalctl|exec 3<&0'
```

`exec 3<&0` repère le script de surveillance qui enveloppe chaque flux. Pendant qu'un flux est ouvert, la commande liste le script, `docker logs -f …` (ou `journalctl …`) et son surveillant. Après chaque arrêt ci-dessous, elle ne doit **rien** renvoyer (attendre 1 à 2 s).

Utiliser `vpsctl-t-quiet` (silencieux) : c'est le cas où un processus survivrait si l'arrêt reposait sur sa prochaine écriture.

- [ ] Flux ouvert sur `vpsctl-t-quiet` : la commande liste les processus du flux.
- [ ] Bouton **Interrompre le flux** : plus rien.
- [ ] **Fermer** le panneau : plus rien.
- [ ] **Changer de source** (Logs sur une autre ligne) : seul le nouveau flux est listé.
- [ ] **Quitter le tableau de bord** (Fichiers) : plus rien. En revenant, la console se rouvre sur la même source, avec un nouveau flux.
- [ ] **Déconnexion** (« Couper la liaison ») : plus rien.
- [ ] **Fermeture de l'application** (bouton Fermer de la barre de titre) : plus rien.
- [ ] **Application tuée** (Windows : gestionnaire des tâches, fin de tâche ; Linux : `kill -9` sur le processus `vpsctl`) : plus rien. Ici, seul le script de surveillance agit : la connexion est coupée sans signal SSH.
- [ ] **Rechargement de l'interface** (Ctrl+R ou F5 dans la fenêtre en développement) : plus rien, puis le tableau de bord revient sans flux ouvert.
- [ ] **Fin d'elle-même** : flux ouvert sur `vpsctl-t-logs`, puis `docker stop -t 1 vpsctl-t-logs` sur le VPS. Après la bannière LOS : plus rien, **y compris aucun shell `exec 3<&0`**. Le surveillant ne survit pas à la commande.
- [ ] Même série avec un service calme (`vpsctl-t-slow.service`, qui n'écrit rien) : Interrompre, Fermer, Déconnexion, application tuée. Plus rien après chacun.

## 8. Rendu Windows et Linux

- [ ] Windows (WebView2) et Linux (WebKitGTK) : colonne d'actions alignée, infobulles, fenêtre GO / NO GO, notifications et console identiques aux captures (`docs/design/ecran/dashboard/`, `telemetrie/`, `go-no-go.png`, `notifications.png`, `interrupteur.png`).
- [ ] Les heures de la console sont correctes sous WebKitGTK (horodatage à la milliseconde, pas de case vide).
- [ ] Défilement fluide de la console sous WebKitGTK avec 5 000 lignes.

## 9. Sécurité (contrôle rapide)

Couvert par les tests unitaires (`cargo test`). Contrôle facultatif depuis les outils de développement de la fenêtre (console JavaScript) :

```js
await window.__TAURI_INTERNALS__.invoke("service_action", { unit: "x; touch /tmp/vpsctl-pwned", action: "start" })
await window.__TAURI_INTERNALS__.invoke("docker_remove", { id: "$(touch /tmp/vpsctl-pwned)" })
await window.__TAURI_INTERNALS__.invoke("logs_start", { source: { systemd: "a\nb.service" }, tail: 10, onEvent: null })
```

- [ ] Chaque appel est refusé avec `InvalidInput` (le dernier peut échouer avant, sur `onEvent`). Sur le VPS, `ls /tmp/vpsctl-pwned` : fichier absent.

## 10. Nettoyage

```sh
# Conteneurs et volumes de test
docker ps -a --filter name=vpsctl-t- --format '{{.ID}}' | xargs -r docker rm -f
docker volume rm vpsctl-t-vol
docker volume rm <volume anonyme noté en préparation>

# Services de test
systemctl --user stop vpsctl-t-prio.service vpsctl-t-svc.service vpsctl-t-slow.service vpsctl-t-crash.service 2>/dev/null
systemctl --user reset-failed 'vpsctl-t-*' 2>/dev/null
systemctl --user unmask vpsctl-t-masked.service
rm -f ~/.config/systemd/user/vpsctl-t-prio.service \
      ~/.config/systemd/user/vpsctl-t-crash.service \
      ~/.config/systemd/user/vpsctl-t-slow.service \
      ~/.config/systemd/user/vpsctl-t-svc.service \
      ~/.config/systemd/user/'vpsctl-t\x2descaped.service'
systemctl --user daemon-reload

# Image alpine, si elle n'était pas présente avant les tests
docker image rm alpine

# Vérification : plus rien de préfixé vpsctl-t-, aucun flux restant
docker ps -a --filter name=vpsctl-t-
docker volume ls --filter name=vpsctl-t-
systemctl --user list-units --all 'vpsctl-t-*'
ls ~/.config/systemd/user/ | grep vpsctl-t-
pgrep -u "$USER" -af 'docker logs|journalctl|exec 3<&0'
```

- [ ] Après le nettoyage, l'appli n'affiche plus aucun objet `vpsctl-t-`, et les compteurs reviennent à l'état de départ noté en préparation.
