# Tests manuels du jalon 8 : tests frontend, cas partagés, perte de liaison

À dérouler sur Windows puis sur Linux, avec un build local (`npm run tauri build`, ou `npm run tauri dev` pour la partie 1). Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure et noter les délais mesurés.

Réglages de détection (voir `docs/04-contrat-commandes.md`, « Perte de la liaison ») : keepalive toutes les 5 s sans réception, perte constatée après 4 intervalles sans réponse, soit **15 à 20 s** après une coupure silencieuse ; **60 s** au plus si un envoi de fichier était en train d'écrire au moment de la coupure. Une fermeture par le serveur (FIN, RST, message de déconnexion) est vue aussitôt.

## Préparation

- Node 24 LTS en local (`node --version`), comme la CI.
- Un chronomètre (celui du téléphone suffit). Le **délai mesuré** va de la coupure à l'apparition du bandeau LOS.
- Un terminal SSH ouvert sur le VPS **depuis une autre machine ou une autre connexion** que celle coupée (console Hetzner à défaut), pour vérifier l'état côté serveur.
- Sur le VPS :
  - un script long enregistré dans Atlas, par exemple « compteur » : `for i in $(seq 1 600); do echo "ligne $i"; sleep 1; done` ;
  - un fichier d'environ 500 Mo à télécharger (`fallocate -l 500M ~/gros.bin`), et un fichier local de même taille à envoyer ;
  - un service de test (`test-atlas.service`) et un conteneur de test, pour les actions.
- Adresse IP du VPS (`IP` ci-dessous), pour les coupures par pare-feu.

### Couper la liaison sans toucher à la carte réseau (coupure silencieuse)

Le plus reproductible : bloquer le VPS au pare-feu local. Aucun paquet ne passe, rien n'est fermé, exactement comme un réseau qui tombe en route.

- Windows (PowerShell administrateur) :
  - couper : `New-NetFirewallRule -DisplayName "atlas-coupure" -Direction Outbound -RemoteAddress IP -Action Block`
  - rétablir : `Remove-NetFirewallRule -DisplayName "atlas-coupure"`
- Linux :
  - couper : `sudo iptables -I OUTPUT -d IP -j DROP`
  - rétablir : `sudo iptables -D OUTPUT -d IP -j DROP`

Débrancher le câble ou couper le Wi-Fi donne parfois une erreur immédiate du système (« réseau injoignable ») au lieu d'un silence : les deux cas sont testés en partie 3.

## 1. Tests automatisés

### Windows

- [ ] `npm ci` puis `npm test` : tous les tests passent (stores, `logSession`, formats, cas partagés).
- [ ] `cd src-tauri && cargo test` : tous les tests passent, dont `link_loss` (serveur SSH local, 9 tests, une vingtaine de secondes) et les modules `shared` des cas partagés.
- [ ] Lancer deux fois de suite `cargo test --test link_loss` : aucun échec intermittent.
- [ ] `npm run lint`, `npm run typecheck`, `npx prettier --check .`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` : rien à signaler.

### Linux

- [ ] Mêmes commandes, mêmes résultats.

### Cas partagés : une divergence fait échouer les deux côtés

- [ ] Dans `tests/shared/service-name.json`, passer le premier cas (`bot-discord`) à `"valid": false`.
- [ ] `npm test` échoue sur ce cas (nom de la justification affiché) ; `cargo test shared` échoue aussi, en listant `"bot-discord" : attendu invalide, obtenu valide`.
- [ ] Remettre `"valid": true` : les deux passent.

### Formulaires alignés sur le backend

- [ ] Scripts : un nom de 64 émojis est accepté par le formulaire et enregistré ; 65 est refusé avant l'envoi (« 64 caractères au plus »).
- [ ] Nouveau service : un dossier de travail `/srv/app` suivi d'un caractère NEL (U+0085, à coller) n'est pas signalé en erreur, et l'aperçu s'affiche.

## 2. Liaison au repos

- [ ] Établir la liaison et ne rien faire pendant **5 minutes**, fenêtre au premier plan puis réduite : aucun LOS, l'indicateur reste AOS, la latence se met à jour toutes les 30 s.
- [ ] **Coupure brève** : couper au pare-feu, attendre **5 s**, rétablir. Aucun LOS ; le tableau de bord reprend son rafraîchissement dans les secondes qui suivent.
- [ ] Coupure de 10 s puis rétablissement : toujours aucun LOS (le seuil est 15 à 20 s).

## 3. Détection de la perte

Pour chaque cas : bandeau LOS au-dessus du contenu, indicateur de la barre de titre « LOS · <profil> · depuis mm:ss » (rouge, antenne barrée), pied de la navigation « Liaison perdue · mm:ss », l'écran affiché ne change pas. Le bandeau donne la durée (qui défile), le dernier échange et la cause.

| Cas | Cause attendue dans le bandeau | Délai attendu | Délai mesuré Windows | Délai mesuré Linux |
|---|---|---|---|---|
| Pare-feu (coupure silencieuse) | le serveur ne répond plus | 15 à 20 s | | |
| Câble débranché / Wi-Fi coupé | le serveur ne répond plus, ou réseau injoignable | 0 à 20 s | | |
| Veille ≈ 1 min puis réveil | aucun LOS si la connexion a survécu ; sinon une des causes ci-dessus | ≤ 20 s après le réveil | | |
| Veille ≥ 15 min puis réveil | connexion réinitialisée par l'hôte, ou le serveur ne répond plus | ≤ 20 s après le réveil | | |
| VPS redémarré (console Hetzner, « Redémarrer ») | connexion fermée par le serveur, ou réinitialisée par l'hôte | quelques secondes | | |
| VPS réinitialisé (« Reset », coupure électrique) — facultatif | le serveur ne répond plus | 15 à 20 s | | |

- [ ] Pare-feu : délai et cause conformes.
- [ ] Câble ou Wi-Fi : délai et cause notés.
- [ ] Veille courte, puis veille longue : comportement noté. Sous Linux, l'horloge de l'application ne compte pas le temps de veille : le délai se compte depuis le réveil.
- [ ] Redémarrage du VPS : délai et cause notés.
- [ ] Le bandeau ne se ferme pas, n'a pas de croix ; « Rétablir la liaison » est le seul bouton d'action. Le contenu sous le bandeau défile, le bandeau reste en place.
- [ ] Un seul bandeau et une seule perte par coupure : aucune notification d'erreur en plus (ni « Échec », ni « Procédure interrompue », ni « Envoi interrompu »).

## 4. Perte pendant une opération

Coupure au pare-feu sauf mention contraire.

### Tableau de bord

- [ ] Pendant le LOS : les deux tableaux restent affichés, contour pointillé, contenu atténué, pastille « FIGÉ · hh:mm:ss » ; bloc d'actualisation « Figé · dernière actualisation hh:mm:ss », « Actualiser » désactivé. Toutes les actions d'une ligne sont grisées (infobulle « liaison perdue »), « Nouveau service » aussi.
- [ ] Les tableaux ne se vident pas, même si une relecture a échoué juste avant la détection (attendre 20 s après la coupure).

### Juste après une action Docker ou systemd

- [ ] Redémarrer le service de test, couper le pare-feu **dans les 2 s** : la ligne quitte « En transition » au LOS, **aucune** notification d'échec. Après rétablissement, l'état réel est affiché (normalement « Nominal »).
- [ ] Même chose avec un démarrage de conteneur.
- [ ] Couper **avant** de cliquer (puis cliquer pendant les 15 s où la perte n'est pas encore vue) : à la détection, aucune notification « Échec du démarrage » ; vérifier sur le VPS si la commande a été reçue (`systemctl --user status test-atlas`).

### Flux de logs

- [ ] Panneau ancré ouvert sur un conteneur actif : au LOS, bannière « LOS · Liaison perdue · flux interrompu · hh:mm:ss » rouge, lignes gardées, bouton « Reprendre » désactivé.
- [ ] Onglet Télémétrie sur les logs d'un service : pastille « INTERROMPU » dans l'onglet, même bannière.
- [ ] Quitter le tableau de bord puis y revenir pendant le LOS : le panneau ne relance rien.

### Transfert

- [ ] Téléchargement de `gros.bin`, couper à mi-parcours : la ligne passe en « Erreur », « LOS à X Mo sur 500 Mo · relance au rétablissement », « Réessayer » désactivé. Délai mesuré : ____ s.
- [ ] Envoi du fichier de 500 Mo, couper à mi-parcours : même affichage. **Délai mesuré : ____ s** (jusqu'à 60 s si l'écriture était bloquée).
- [ ] Un second transfert mis en file avant la coupure : « En attente · File suspendue pendant la coupure », « Retirer » disponible.
- [ ] Le panneau des transferts a un contour rouge pendant le LOS.
- [ ] Trois transferts en file (envoi du fichier de 500 Mo, puis deux téléchargements). Couper au pare-feu pendant le premier. Attendu : le premier passe en erreur (« LOS à X Mo… »), sans notification ; les deux autres restent « En attente · File suspendue pendant la coupure » ; **aucun** ne démarre, même pendant les 20 s qui précèdent le bandeau. Délai mesuré : ____ s.
- [ ] Même scénario en coupant pendant que l'envoi écrit (débit maximal) : si une erreur réseau arrive avant le bandeau, la file se suspend aussitôt (« File suspendue après une erreur réseau »), aucun deuxième transfert n'échoue.
- [ ] Après « Rétablir la liaison » : rien ne repart ; « Reprendre la file » lance le deuxième transfert, puis le troisième. Un envoi qui devait écraser un fichier redemande le GO / NO GO.

### Script

- [ ] Lancer « compteur », couper vers la ligne 30 : pastille « ISSUE INCONNUE » dans l'onglet, en-tête « Liaison perdue · issue inconnue », séparateur « LOS · Liaison perdue · issue inconnue · hh:mm:ss » puis le message « Le script a été interrompu sur le serveur, probablement en cours d'exécution. Vérifiez l'état du serveur avant de le relancer. » « Relancer » désactivé ; l'écran Scripts a ses boutons « Exécuter » désactivés.
- [ ] Sur le VPS, noter si `compteur` tourne encore (`pgrep -af seq`) : il n'est arrêté que lorsque sshd constate la coupure (sans `ClientAliveInterval`, cela peut prendre longtemps). Relevé : ____.
- [ ] Changer d'écran pendant le LOS puis revenir : l'onglet est toujours là, rien n'a été relancé.

### Explorateur

- [ ] Pendant le LOS : liste figée (pointillés, atténuée, « FIGÉ · hh:mm:ss » dans l'itinéraire), étapes de l'itinéraire, « Nouveau dossier », « Nouveau fichier », « Téléverser », actualisation et ouverture d'un dossier inactifs. « Copier le chemin » fonctionne.

## 5. Déconnexion volontaire : jamais de LOS

- [ ] « Couper la liaison » (avec un script en cours : GO / NO GO) : retour direct à l'écran de connexion, aucun bandeau LOS, aucune notification d'erreur.
- [ ] Couper la liaison puis couper le pare-feu juste après : toujours aucun LOS.
- [ ] Pendant un LOS, « Couper la liaison » : retour à l'écran de connexion, transferts et onglets vidés.
- [ ] Fermer l'application pendant une liaison : rien ne s'affiche au relancement (pas de LOS hérité).

## 6. Reconnexion manuelle

- [ ] Pendant le LOS, pare-feu toujours fermé : « Rétablir la liaison » affiche « ACQ · Rétablissement de la liaison… · Tentative 1 · user@hôte:port · mm:ss · coupure de mm:ss » (contour pointillé ambre dans la barre de titre), « Annuler » interrompt et revient au LOS. Laisser échouer : retour au LOS avec « Tentative 1 échouée : … ».
- [ ] Un deuxième clic affiche « Tentative 2 » : le compteur compte les clics, aucune tentative ne part seule (attendre 2 minutes sans cliquer pour le vérifier).
- [ ] Passphrase non mémorisée (la retirer du trousseau avant le test) : « Rétablissement suspendu · passphrase requise », le panneau « Déverrouiller la clé privée » s'ouvre par-dessus l'écran. « Annuler » revient au LOS ; la bonne passphrase rétablit la liaison.
- [ ] Rouvrir le pare-feu puis « Rétablir la liaison » : bandeau vert « AOS · Liaison rétablie · Coupure de mm:ss · données actualisées à hh:mm:ss » (plus « flux de journal à reprendre dans la console » si des logs étaient ouverts), refermé après 5 s ou par « Fermer ». Indicateur « AOS » et latence.

### Tout refonctionne

- [ ] Tableau de bord : listes relues aussitôt, rafraîchissement toutes les 5 s, actions possibles (redémarrer le service de test : « En transition » puis « Nominal »).
- [ ] Logs : « Reprendre » (bouton principal) relance le flux à la suite des anciennes lignes, avec un avis « Flux interrompu à hh:mm:ss, repris à hh:mm:ss · lignes de l'intervalle non récupérées » ; les lignes émises pendant la coupure n'apparaissent pas.
- [ ] Script : séparateur neutre « Script interrompu à hh:mm:ss · pas de code de sortie », message conservé ; « Relancer » (avec GO / NO GO si le script le demande) lance une nouvelle exécution. Écran Scripts : la dernière exécution est « interrompue » (pas de code de sortie).
- [ ] Transferts : rien n'est reparti seul. « Réessayer » sur le transfert coupé repart de 0 %. « Reprendre la file » lance le transfert retenu. Un nouvel envoi lancé après le rétablissement part directement, sans libérer la file retenue.
- [ ] Explorateur : dossier relu ; le temporaire d'un envoi coupé apparaît avec la pastille « Résidu de transfert » ; le supprimer fonctionne (session SFTP neuve).
- [ ] Nouveau flux de logs, nouveau script, nouveau transfert : la limite de 4 flux simultanés est intacte (les flux coupés ne comptent plus).

## 7. Relevé des délais

| Cas | Windows | Linux |
|---|---|---|
| Pare-feu, liaison au repos | | |
| Pare-feu, pendant un flux de logs | | |
| Pare-feu, pendant un téléchargement | | |
| Pare-feu, pendant un envoi | | |
| Câble ou Wi-Fi | | |
| Veille puis réveil | | |
| Redémarrage du VPS | | |
