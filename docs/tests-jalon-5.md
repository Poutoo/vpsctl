# Tests manuels du jalon 5 : scripts et télémétrie

À dérouler sur Windows puis sur Linux. Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure.

Tous les scripts de test sont créés dans l'application avec un nom préfixé `vpsctl-t-`, et les objets côté serveur dans `~/vpsctl-t-*` ; tout est supprimé à la fin (section 15).

Les scripts sont enregistrés dans :

- Windows : `%APPDATA%\dev.poutoo.atlas\scripts.json`
- Linux : `~/.config/dev.poutoo.atlas/scripts.json`

## Préparation

- Lancer l'application : `npm run tauri dev`, se connecter au VPS.
- Garder un terminal SSH ouvert sur le VPS, avec le même compte, pour les vérifications côté serveur.
- Noter le shell de connexion du compte : `getent passwd "$USER" | cut -d: -f7`. Les sections 6 et 7 supposent bash (Ubuntu) ; avec zsh, remplacer `~/.profile` par `~/.zprofile` et `~/.bashrc` par `~/.zshrc`.

### Fichiers de test sur le VPS

```sh
# Sauvegarde : ~/.profile et ~/.bashrc sont modifiés en sections 6 et 7
cp ~/.profile ~/.profile.vpsctl-t.bak
cp ~/.bashrc ~/.bashrc.vpsctl-t.bak

# 1. Commande accessible uniquement par le PATH du shell de connexion
mkdir -p ~/vpsctl-t-bin
printf '#!/bin/sh\necho "PATH du shell de connexion : OK"\n' > ~/vpsctl-t-bin/vpsctl-t-hello
chmod +x ~/vpsctl-t-bin/vpsctl-t-hello
echo 'PATH="$HOME/vpsctl-t-bin:$PATH" # vpsctl-t' >> ~/.profile

# 2. Réglage placé dans ~/.bashrc (après sa garde « non interactif »), comme nvm
echo 'export VPSCTL_T_BASHRC=charge # vpsctl-t' >> ~/.bashrc

# 3. Le même réglage dans un fichier à charger explicitement
echo 'export VPSCTL_T_BASHRC=charge' > ~/vpsctl-t-env.sh
```

Vérification préalable dans le terminal SSH, en mode non interactif comme l'appli : `ssh <vps> 'bash -l -c vpsctl-t-hello'` affiche « PATH du shell de connexion : OK ».

### Scripts à créer dans l'application

Créer chaque script avec Scripts → Nouveau script. « GO / NO GO » : case « Confirmation GO / NO GO obligatoire » cochée.

| Nom | Confirmation | Commande |
|---|---|---|
| `vpsctl-t-ok` | Directe | `echo début; sleep 1; echo fin` |
| `vpsctl-t-echec` | GO / NO GO | `echo "va échouer"; exit 1` |
| `vpsctl-t-stderr` | Directe | voir ci-dessous |
| `vpsctl-t-multi` | GO / NO GO | voir ci-dessous |
| `vpsctl-t-long` | Directe | `for i in $(seq 1 120); do echo $i; sleep 1; done` |
| `vpsctl-t-read` | Directe | `read x; echo "read : code $?"; read y` |
| `vpsctl-t-sudo` | Directe | `sudo -k true` |
| `vpsctl-t-path` | Directe | `vpsctl-t-hello` |
| `vpsctl-t-bashrc` | Directe | `echo "bashrc : ${VPSCTL_T_BASHRC:-non charge}"` |
| `vpsctl-t-env` | Directe | `. ~/vpsctl-t-env.sh; echo "fichier : ${VPSCTL_T_BASHRC:-non charge}"` |
| `vpsctl-t-pwd` | Directe | `pwd` |
| `vpsctl-t-enfants` | Directe | voir ci-dessous |

`vpsctl-t-stderr` :

```sh
echo "sur la sortie standard"
echo "sur la sortie d'erreur" >&2
ls /vpsctl-t-introuvable
```

`vpsctl-t-multi` (à coller tel quel, sur plusieurs lignes) :

```sh
printf '%s\n' "l'apostrophe" 'guillemets "doubles"' '$HOME non développé'
echo "développé : $HOME"
for x in un deux; do
  echo "boucle $x ; && | > <"
done
echo `echo backtick` $(echo dollar) é ü —
exit 3
```

`vpsctl-t-enfants` (processus en arrière-plan, pour la section 8) :

```sh
sleep 600 &
nohup sleep 601 >/dev/null 2>&1 &
( sleep 602 ) &
echo "enfants lancés"
sleep 603
```

## 1. Liste et formulaire

- [ ] Liste vide : état « 0 PROCÉDURE · Aucun script » avec un bouton Nouveau script.
- [ ] Formulaire : Nom, Description (facultative), Commande en monospace sur plusieurs lignes, sans retour à la ligne automatique (une longue ligne défile horizontalement), case GO / NO GO.
- [ ] Sous le champ Commande, l'aide discrète : « Exécutée dans un shell de connexion : ~/.profile est chargé, mais pas les réglages de ~/.bashrc (nvm, alias…). »
- [ ] Enregistrer avec un nom vide, puis une commande vide (ou faite d'espaces et de retours à la ligne) : message sous le champ, rien n'est enregistré.
- [ ] Nom de plus de 64 caractères : refusé.
- [ ] Annuler : retour à la liste sans changement.
- [ ] Après la création des scripts de test, la liste suit les captures (`docs/design/ecran/scripts/accueil.png`) : nom, description sur deux lignes au plus (complète en infobulle), « ⚠ GO / NO GO » en ambre ou « Directe », « ○ jamais exécutée », boutons Exécuter, Modifier, Supprimer…. Le compteur en haut indique « 12 scripts ».
- [ ] Modifier `vpsctl-t-multi` : le formulaire « MODIFIER · vpsctl-t-multi » montre la commande intacte, retours à la ligne compris.
- [ ] `scripts.json` : la commande de `vpsctl-t-multi` contient des `\n` et aucun `\r` (y compris sous Windows).

## 2. Exécutions courtes

- [ ] Exécuter `vpsctl-t-ok` (Directe) : aucune fenêtre GO / NO GO ; l'écran Télémétrie s'ouvre sur un onglet « vpsctl-t-ok » ; la navigation surligne **Scripts**. En-tête « PROCÉDURE vpsctl-t-ok echo début; sleep 1; echo fin ». Lignes `out` « début » puis « fin », horodatées à la milliseconde.
- [ ] Fin : bannière « LOS · Fin du flux · exit 0 · HH:MM:SS » ; notification de succès « Procédure terminée · vpsctl-t-ok · exit 0 · 1,… s », fermée après 5 s ; l'onglet passe au cercle plein ; le bouton devient « Relancer ».
- [ ] Retour à Scripts : « ● à l'instant · exit 0 » ; après quelques minutes, « il y a N min ».
- [ ] Exécuter `vpsctl-t-echec` : fenêtre GO / NO GO « Exécuter la procédure ? », sujet `vpsctl-t-echec`, avec dans les conséquences « Commande : » et la commande en monospace. NO GO (ou Échap) : rien ne se lance.
- [ ] GO : bannière LOS en rouge « exit 1 » ; notification d'erreur persistante « Procédure en échec · vpsctl-t-echec · exit 1 » avec « Voir la sortie » ; onglet avec le triangle. Dans Scripts : « ▲ … · exit 1 » en rouge.
- [ ] Exécuter `vpsctl-t-stderr` : « sur la sortie standard » en `out`, « sur la sortie d'erreur » et le message de `ls` en `err` **neutre** (pas de rouge) ; compteur « 3 lignes · 2 sur stderr » ; exit 2, notification d'échec.
- [ ] Exécuter `vpsctl-t-multi` : la fenêtre GO / NO GO montre les sept lignes de la commande, telles quelles. Sortie attendue, exactement :

  ```
  l'apostrophe
  guillemets "doubles"
  $HOME non développé
  développé : /home/<compte>
  boucle un ; && | > <
  boucle deux ; && | > <
  backtick dollar é ü —
  ```

  puis LOS « exit 3 ».
- [ ] Exécuter `vpsctl-t-pwd` : affiche le dossier personnel (`/home/<compte>`).
- [ ] Relancer (bouton de la console) `vpsctl-t-echec` : GO / NO GO à nouveau ; GO : même onglet, console vidée, nouvelle exécution.
- [ ] Notification « Voir la sortie » cliquée depuis un autre écran : l'écran Télémétrie s'ouvre sur l'onglet du script.

## 3. Script long et changement d'écran

- [ ] Exécuter `vpsctl-t-long`. Le bouton Exécuter de la liste (revenir sur Scripts) affiche un chrono « T+… » et la dernière exécution indique « ◐ en cours ».
- [ ] Hors de l'écran Télémétrie, la navigation affiche au-dessus de « Liaison depuis » : « ◐ 1 procédure en cours · T+00:00:… », le chrono avance. Un clic ouvre l'onglet du script.
- [ ] Pendant l'exécution, aller successivement sur Fichiers, Tableau de bord (ouvrir les logs d'un conteneur dans le panneau), Réglages, attendre 20 s, puis revenir sur l'onglet du script : les nombres sont **consécutifs**, sans trou (aucune ligne perdue pendant l'absence), et la console reprend à la dernière ligne.
- [ ] Changer d'onglet de télémétrie (ouvrir un flux de logs en plein écran, section 9) puis revenir : idem.
- [ ] Réduire la fenêtre 20 s, la restaurer : idem.
- [ ] Cliquer Exécuter sur `vpsctl-t-long` pendant qu'il tourne : l'onglet s'ouvre, **aucune** seconde exécution (une seule suite de nombres ; sur le VPS, `pgrep -u "$USER" -af 'seq 1 120'` ne liste qu'un processus de boucle).
- [ ] À la fin (120 lignes) : LOS « exit 0 », notification de succès avec une durée d'environ « 2 min 00 s ».

## 4. Saisie impossible

- [ ] Exécuter `vpsctl-t-read` : se termine **aussitôt** (pas d'attente), affiche « read : code 1 », LOS « exit 1 ».
- [ ] Exécuter `vpsctl-t-sudo` : se termine aussitôt avec, en `err`, « sudo: a terminal is required to read the password… » (ou équivalent) et « exit 1 ». Aucune invite de mot de passe, nulle part.

## 5. Variables du shell de connexion

- [ ] Exécuter `vpsctl-t-path` : « PATH du shell de connexion : OK », exit 0. La commande n'est trouvée que grâce à `~/.profile`.

## 6. ~/.bashrc non chargé

- [ ] Exécuter `vpsctl-t-bashrc` : « bashrc : non charge ». Le réglage ajouté à la fin de `~/.bashrc` n'est **pas** disponible, comme l'indique l'aide du formulaire.
- [ ] Exécuter `vpsctl-t-env` : « fichier : charge ». Charger explicitement un fichier (comme `. "$HOME/.nvm/nvm.sh"` pour nvm) est la solution.
- [ ] Si nvm est installé : un script `node --version` échoue (« node: not found », code 127) ; `. "$HOME/.nvm/nvm.sh"; node --version` affiche la version.

## 7. Interruption et processus restants

Commande de vérification, sur le VPS :

```sh
pgrep -u "$USER" -af 'exec 3<&0|seq 1 120|sleep 60[0-3]'
```

`exec 3<&0` repère le script de surveillance qui enveloppe chaque exécution.

- [ ] `vpsctl-t-long` en cours : la commande liste l'enveloppe, le shell de connexion, la boucle et son `sleep 1`.
- [ ] Bouton « Interrompre le flux » : fenêtre GO / NO GO « Interrompre la procédure ? » (SIGTERM, rien n'est annulé, exécution enregistrée comme interrompue). NO GO : le script continue.
- [ ] GO : bannière « LOS · Procédure interrompue », onglet au cercle vide, **aucune** notification. Après 1 à 2 s, la commande de vérification ne renvoie **rien**.
- [ ] Scripts : « ○ à l'instant · interrompue ».
- [ ] Exécuter `vpsctl-t-enfants` : « enfants lancés » ; la vérification liste les quatre `sleep 600` à `603`. Interrompre (GO) : plus **rien**, y compris le `nohup` et le sous-shell.
- [ ] Fermer l'onglet d'un script en cours (`vpsctl-t-long`) : fenêtre « Fermer l'onglet ? » avec « Le script sera interrompu ». NO GO : l'onglet reste, le script continue. GO : l'onglet disparaît, l'onglet voisin s'affiche, plus aucun processus.
- [ ] Fermer l'onglet d'un script **terminé** : aucune fenêtre.
- [ ] Limite connue (facultatif) : un script `setsid sleep 604` quitte le groupe de processus et survit à l'interruption. Le vérifier avec `pgrep -af 'sleep 604'`, puis `pkill -f 'sleep 604'`.

## 8. Déconnexion et fermeture pendant un script

- [ ] Aucun script en cours : « Couper la liaison » déconnecte sans fenêtre ; fermer l'application (bouton Fermer) la ferme sans fenêtre.
- [ ] `vpsctl-t-long` en cours, « Couper la liaison » : fenêtre « Couper la liaison ? », sujet `vpsctl-t-long`, « 1 script en cours sera interrompu ». NO GO : la liaison reste, le script continue.
- [ ] Deux scripts en cours (`vpsctl-t-long` et `vpsctl-t-enfants`) : « 2 scripts en cours seront interrompus », les deux noms en sujet.
- [ ] GO : retour à l'écran de connexion ; sur le VPS, plus aucun processus. Après reconnexion : aucun onglet ouvert, et dans Scripts les deux affichent « interrompue ».
- [ ] `vpsctl-t-long` en cours, bouton **Fermer** de la barre de titre : fenêtre « Fermer l'application ? ». NO GO : l'application reste ouverte, le script continue (les nombres avancent).
- [ ] Même chose avec **Alt+F4** (Windows) et, sous Linux, la fermeture par le gestionnaire de fenêtres (raccourci ou menu de la barre des tâches) : la fenêtre GO / NO GO s'affiche aussi.
- [ ] GO : l'application se ferme ; sur le VPS, plus aucun processus. Au redémarrage de l'application : `vpsctl-t-long` « interrompue ».
- [ ] Application tuée pendant un script (gestionnaire des tâches ; `kill -9` sous Linux) : sur le VPS, plus aucun processus (le surveillant agit). Au redémarrage, la dernière exécution n'a **pas** été enregistrée (l'application n'a pas pu l'écrire) : c'est attendu.

## 9. Panneau ancré et onglets de logs

- [ ] Tableau de bord, Logs d'un conteneur : le panneau ancré s'ouvre comme au jalon 3, avec un bouton « Plein écran » en plus.
- [ ] Plein écran : l'écran Télémétrie s'ouvre sur un nouvel onglet du conteneur, avec **les mêmes lignes** (pas de rechargement de l'historique, pas de bannière LOS) ; le flux continue. La navigation surligne **Tableau de bord**. Retour au tableau de bord : plus de panneau.
- [ ] Logs du **même** conteneur depuis le tableau de bord : l'onglet existant s'ouvre, pas de second flux.
- [ ] Onglet de logs, aller sur Fichiers puis revenir : le flux a continué (nouvelles lignes reçues pendant l'absence).
- [ ] Onglets : forme d'état par onglet (cercle plein en direct, cercle vide terminé, demi-disque pour un script en cours, triangle en échec), onglet actif en ambre ; un nom long est coupé par « … », nom complet en infobulle.
- [ ] Fermer un onglet de logs : aucune fenêtre, le flux s'arrête (`pgrep -u "$USER" -af 'docker logs|journalctl'` ne le liste plus).
- [ ] Écran Télémétrie sans onglet : état « 0 FLUX · Aucun flux ouvert » avec des liens vers Tableau de bord et Scripts.
- [ ] Indicateur de la navigation sans script en cours mais avec des onglets de logs : « ● Télémétrie · N flux », qui mène au premier onglet.

## 10. Limite de 4 flux

- [ ] Ouvrir 3 onglets de logs (Logs puis Plein écran, trois fois) et garder un 4ᵉ flux dans le panneau ancré. Exécuter `vpsctl-t-ok` : notification d'erreur « Script non lancé · vpsctl-t-ok » avec le message « 4 flux déjà ouverts : fermez un onglet de télémétrie pour lancer ce script ». Aucun onglet n'est créé, la dernière exécution n'est pas modifiée.
- [ ] Fermer un onglet, relancer : le script s'exécute.

## 11. Suppression

- [ ] Supprimer… sur `vpsctl-t-pwd` : fenêtre GO / NO GO rouge « Supprimer le script ? » (retiré de l'application, rien n'est modifié sur le serveur). NO GO : rien ne change. GO : il disparaît de la liste et de `scripts.json`.
- [ ] Supprimer `vpsctl-t-long` **pendant** son exécution : la fenêtre ajoute « Son exécution en cours continue jusqu'à sa fin ; elle ne sera pas enregistrée ». GO : le script disparaît, l'onglet continue jusqu'au bout. Puis « Relancer » : notification « Script non lancé » (script introuvable), rien ne s'exécute. Recréer `vpsctl-t-long` pour la suite si besoin.
- [ ] Modifier un script exécuté : sa dernière exécution est conservée.

## 12. Rechargement de l'interface

- [ ] `vpsctl-t-long` en cours, Ctrl+R ou F5 (fenêtre en développement) : l'interface se recharge **sans** confirmation, le script est interrompu (aucun processus restant), plus aucun onglet ; la dernière exécution indique « interrompue ». Limite connue : pas de GO / NO GO dans ce cas.

## 13. Rendu Windows et Linux

- [ ] Windows (WebView2) et Linux (WebKitGTK) : écran Scripts, formulaire, fenêtre GO / NO GO avec commande sur plusieurs lignes (défilante au-delà de 8 lignes), onglets et console identiques aux captures (`docs/design/ecran/scripts/`, `telemetrie/`).
- [ ] Description sur deux lignes au plus, coupée par « … » (`line-clamp`), sous les deux moteurs.
- [ ] Fenêtre étroite : la navigation se réduit aux numéraux ; l'indicateur ne montre plus que sa forme, avec une infobulle « 1 procédure en cours · T+… ».

## 14. Sécurité (contrôle rapide)

Couvert par les tests unitaires (`cargo test`, dont l'exécution réelle par `sh` sous Linux). Contrôle facultatif depuis les outils de développement de la fenêtre (console JavaScript) :

```js
await window.__TAURI_INTERNALS__.invoke("scripts_save", { script: { id: "", name: "x", description: "", command: "echo a\u0000b", requireConfirmation: false, lastRun: null } })
await window.__TAURI_INTERNALS__.invoke("scripts_save", { script: { id: "inconnu", name: "x", description: "", command: "echo", requireConfirmation: false, lastRun: null } })
await window.__TAURI_INTERNALS__.invoke("script_run", { id: "$(touch /tmp/vpsctl-pwned)", onEvent: null })
```

- [ ] Chaque appel est refusé avec `InvalidInput` (le dernier peut échouer avant, sur `onEvent`). Sur le VPS, `ls /tmp/vpsctl-pwned` : fichier absent.
- [ ] `lastRun` forgé : enregistrer un script en passant `lastRun: { at: 1, exitCode: 42 }` par la même console ; `scripts_list` renvoie la valeur précédente (ou `null`), jamais celle-ci.

## 15. Nettoyage

Dans l'application : supprimer tous les scripts `vpsctl-t-*`.

Sur le VPS :

```sh
# Réglages ajoutés à ~/.profile et ~/.bashrc
sed -i '/# vpsctl-t$/d' ~/.profile ~/.bashrc
diff ~/.profile ~/.profile.vpsctl-t.bak && diff ~/.bashrc ~/.bashrc.vpsctl-t.bak \
  && rm ~/.profile.vpsctl-t.bak ~/.bashrc.vpsctl-t.bak

# Fichiers de test
rm -rf ~/vpsctl-t-bin ~/vpsctl-t-env.sh

# Vérification : aucun processus restant
pgrep -u "$USER" -af 'exec 3<&0|seq 1 120|sleep 60[0-4]|docker logs|journalctl'
```

- [ ] Les deux `diff` ne renvoient rien (fichiers d'origine retrouvés), aucun processus restant, et la liste des scripts de l'application est vide (ou revenue à l'état de départ).
