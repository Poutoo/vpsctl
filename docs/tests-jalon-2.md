# Tests manuels du jalon 2 : conteneurs et services en lecture seule

À dérouler sur Windows puis sur Linux. Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure.

Tous les objets de test sont préfixés `vpsctl-t-` pour être nettoyés facilement (section 7).

## Préparation

- Lancer l'application : `npm run tauri dev`, se connecter au VPS, ouvrir le tableau de bord.
- Pour les sections 5 et 6 : ouvrir les outils de développement de la fenêtre (clic droit → Inspecter), onglet Réseau. Chaque appel au backend y apparaît comme une requête `ipc://` nommée d'après la commande (`docker_list`, `services_list`).
- Noter sur le VPS l'état de départ, pour comparer :

```sh
docker ps -a --format '{{.Names}}\t{{.State}}\t{{.Status}}'
systemctl --user list-units --type=service --all
ls -la ~/.config/systemd/user/
```

### Cas de test Docker

```sh
# Sorti en code 0 → « Arrêté · code 0 »
docker run --name vpsctl-t-exit0 alpine true

# Sorti en code 1 → « En échec · code 1 »
docker run --name vpsctl-t-exit1 alpine sh -c 'exit 1'

# Arrêté par docker stop → « Arrêté · code 137 »
# (sleep est le PID 1 et ignore SIGTERM : Docker le tue après le délai, code 137)
docker run -d --name vpsctl-t-stop alpine sleep 1d
docker stop -t 2 vpsctl-t-stop

# En cours → « Nominal », « depuis … »
docker run -d --name vpsctl-t-run alpine sleep 1d

# Nom long, pour la troncature
docker run --name vpsctl-t-un-nom-de-conteneur-volontairement-tres-long-pour-tester-la-troncature alpine true
```

### Cas de test systemd

```sh
mkdir -p ~/.config/systemd/user

# Service qui échoue → « En échec »
cat > ~/.config/systemd/user/vpsctl-t-fail.service <<'EOF'
[Unit]
Description=vpsctl test : échec volontaire

[Service]
ExecStart=/bin/false
EOF

# Gabarit et son instance → seule l'instance vpsctl-t-tpl@1.service apparaît
cat > ~/.config/systemd/user/vpsctl-t-tpl@.service <<'EOF'
[Unit]
Description=vpsctl test : instance %i

[Service]
ExecStart=/bin/sleep infinity
EOF

# Jamais démarré → « Arrêté », sans description
cat > ~/.config/systemd/user/vpsctl-t-idle.service <<'EOF'
[Unit]
Description=vpsctl test : jamais démarré

[Service]
ExecStart=/bin/sleep infinity
EOF

systemctl --user daemon-reload
systemctl --user start vpsctl-t-fail.service
systemctl --user start vpsctl-t-tpl@1.service

# Unité masquée → « Arrêté », mention « masqué »
systemctl --user mask vpsctl-t-masked.service

# Nom hors du format pilotable (contient une barre oblique inverse) → mention « non pilotable »
cat > ~/.config/systemd/user/'vpsctl-t\x2descaped.service' <<'EOF'
[Unit]
Description=vpsctl test : nom échappé

[Service]
ExecStart=/bin/sleep infinity
EOF
systemctl --user daemon-reload
```

## 1. Conteneurs Docker

- [ ] La section « Conteneurs Docker » liste **tous** les conteneurs, y compris arrêtés, triés par nom. Les compteurs (ex. « 2 nominaux · 3 arrêtés · 1 en échec ») correspondent aux lignes.
- [ ] `vpsctl-t-exit0` : cercle vide, « Arrêté · code 0 », durée « il y a … ».
- [ ] `vpsctl-t-exit1` : triangle rouge, « En échec · code 1 ».
- [ ] `vpsctl-t-stop` : cercle vide, « Arrêté · code 137 ».
- [ ] `vpsctl-t-run` : cercle plein, « Nominal », sans code, durée « depuis … » (ex. « depuis environ une minute », puis « depuis 2 minutes »).
- [ ] Les durées sont en français ; leur infobulle donne le texte brut de Docker (`Exited (137) 2 minutes ago`).
- [ ] L'infobulle du libellé d'état donne l'état brut (`exited · Exited (1) …`).
- [ ] Le nom long est tronqué avec « … » ; l'infobulle affiche le nom complet. Même chose pour une image longue.
- [ ] Une image référencée par digest (`image@sha256:…`) s'affiche sans le digest.
- [ ] Les états et durées correspondent à `docker ps -a` sur le VPS.

## 2. Services systemd

- [ ] La section « Services systemd » ne liste que des services dont le fichier est dans `~/.config/systemd/user/`. Aucun service du système (`dbus.service`, `pipewire.service`…) alors que `systemctl --user list-units --type=service --all` en affiche.
- [ ] `vpsctl-t-fail.service` : triangle, « En échec », description « vpsctl test : échec volontaire ».
- [ ] `vpsctl-t-tpl@1.service` : « Nominal », description « vpsctl test : instance 1 ». `vpsctl-t-tpl@.service` (le gabarit) n'apparaît **pas**.
- [ ] `vpsctl-t-idle.service` : « Arrêté », description « — » (infobulle « Unité non chargée par systemd »), infobulle de l'état `non chargé · inactive/dead`.
- [ ] `vpsctl-t-masked.service` : « Arrêté », mention « masqué ».
- [ ] `vpsctl-t\x2descaped.service` : affiché, avec la mention « non pilotable ».
- [ ] L'infobulle du libellé d'état donne l'état brut (`loaded · failed/failed`).
- [ ] Les compteurs correspondent aux lignes.
- [ ] `systemctl --user start vpsctl-t-tpl@2.service` sur le VPS : l'instance apparaît au rafraîchissement suivant, sans action dans l'appli.

## 3. Sections indépendantes et erreurs

- [ ] Permission Docker refusée (facultatif, demande des droits root sur le VPS) : `sudo gpasswd -d <utilisateur> docker`, puis se déconnecter et se reconnecter dans l'appli. La section Docker affiche « Accès à Docker refusé : le compte SSH doit appartenir au groupe docker. », sans liste. La section systemd reste remplie et continue de se rafraîchir. Rétablir : `sudo usermod -aG docker <utilisateur>`, se reconnecter : la liste revient.
- [ ] Pendant un chargement initial (juste après la connexion), chaque section affiche « Chargement… » indépendamment.
- [ ] Liste vide : sur un serveur sans conteneur (ou après le nettoyage, s'il n'y en avait pas d'autres), « Aucun conteneur sur ce serveur. ». Sans fichier dans `~/.config/systemd/user/` : « Aucun service dans ~/.config/systemd/user/. ».

## 4. Rafraîchissement automatique

- [ ] L'indication « Actualisé à HH:MM:SS · il y a N s » s'affiche ; N augmente chaque seconde et repasse à 0 environ toutes les 5 secondes (5 s plus la durée d'un rafraîchissement).
- [ ] Sur le VPS, `docker stop -t 2 vpsctl-t-run` : la ligne passe en « Arrêté · code 137 » sans action dans l'appli, en moins de 10 secondes.
- [ ] Bouton « Actualiser » : l'heure se met à jour immédiatement.

## 5. Rafraîchissement limité au tableau de bord et à la fenêtre visible

Avec l'onglet Réseau des outils de développement ouvert :

- [ ] Sur le tableau de bord : une paire `docker_list` / `services_list` environ toutes les 5 secondes.
- [ ] Aller dans Fichiers ou Scripts pendant 20 secondes : **aucun** appel. Revenir : un appel immédiat, puis toutes les 5 secondes.
- [ ] Réduire la fenêtre pendant 20 secondes : **aucun** appel. La restaurer : un appel immédiat. À vérifier sous WebView2 (Windows) **et** WebKitGTK (Linux).
- [ ] Linux : passer sur un autre bureau virtuel pendant 20 secondes. Noter si les appels s'arrêtent (dépend du gestionnaire de fenêtres).
- [ ] Après « Déconnexion » puis reconnexion à un profil, le tableau de bord n'affiche jamais, même brièvement, les listes de la liaison précédente.

## 6. Pas de chevauchement

- [ ] Cliquer rapidement une dizaine de fois sur « Actualiser » : dans l'onglet Réseau, jamais deux `docker_list` (ni deux `services_list`) en cours en même temps.
- [ ] Cliquer sur « Actualiser » juste avant un rafraîchissement automatique : pas d'appel en double.

## 7. Nettoyage

```sh
# Conteneurs de test
docker ps -a --filter name=vpsctl-t- --format '{{.ID}}' | xargs -r docker rm -f

# Services de test
systemctl --user stop 'vpsctl-t-tpl@1.service' 'vpsctl-t-tpl@2.service' 2>/dev/null
systemctl --user reset-failed 'vpsctl-t-*' 2>/dev/null
systemctl --user unmask vpsctl-t-masked.service
rm -f ~/.config/systemd/user/vpsctl-t-fail.service \
      ~/.config/systemd/user/vpsctl-t-tpl@.service \
      ~/.config/systemd/user/vpsctl-t-idle.service \
      ~/.config/systemd/user/'vpsctl-t\x2descaped.service'
systemctl --user daemon-reload

# Image alpine, si elle n'était pas présente avant les tests
docker image rm alpine

# Vérification : plus rien de préfixé vpsctl-t-
docker ps -a --filter name=vpsctl-t-
systemctl --user list-units --all 'vpsctl-t-*'
ls ~/.config/systemd/user/ | grep vpsctl-t-
```

- [ ] Après le nettoyage, l'appli n'affiche plus aucun objet `vpsctl-t-`, et les compteurs reviennent à l'état de départ noté en préparation.
