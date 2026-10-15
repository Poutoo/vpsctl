# Tests manuels du jalon 1 : connexion SSH

À dérouler sur Windows puis sur Linux. Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure.

## Préparation

- Lancer l'application : `npm run tauri dev`.
- Fichier des profils :
  - Windows : `%APPDATA%\dev.poutoo.atlas\profiles.json`
  - Linux : `~/.config/dev.poutoo.atlas/profiles.json`
- Trousseau : entrée `passphrase:<id du profil>` du service `dev.poutoo.atlas`.
  - Windows : Gestionnaire d'identification → Informations d'identification Windows.
  - Linux : Seahorse (« Mots de passe et clés ») ou `secret-tool search --all service dev.poutoo.atlas`.
- Avoir sous la main l'empreinte du serveur, lue sur la console de l'hébergeur : `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`.
- Noter la date de modification de `~/.ssh/known_hosts` (si le fichier existe) : elle ne doit jamais changer pendant ces tests.
- Utiliser une clé **chiffrée par passphrase** pour la section 4.
- Créer, dans Réglages, trois profils de test (chaque profil a sa propre empreinte mémorisée) :
  - `vps-test` : hôte, port 22, utilisateur et clé corrects ;
  - `vps-mauvais-port` : comme `vps-test`, port `2223` ;
  - `vps-mauvais-user` : comme `vps-test`, utilisateur `inexistant`.

## 1. Première connexion

- [y] Sur l'écran de connexion, sélectionner `vps-test` puis « Établir la liaison ».
- [y] Télémétrie : `TCP ✓ établie · N ms`, `SSH ✓` avec les algorithmes négociés (ex. `mlkem768x25519-sha256 · chacha20-poly1305@openssh.com`), `HÔTE ?` clé inconnue, puis « LIAISON fermée avant authentification ». Aucune ligne `AUTH`.
- [y] Le panneau « Comparez l'empreinte du serveur » affiche l'empreinte en 11 groupes numérotés (01 à 11) et en chaîne continue.
- [y] La chaîne continue est **identique** à la sortie de `ssh-keygen -lf` (`SHA256:` puis 43 caractères, sans `=` final).
- [y] « Refuser » : retour à l'état « En attente », le fichier des profils n'a pas d'empreinte pour `vps-test`.
- [y] Recommencer puis « L'empreinte correspond, accepter » : une nouvelle séquence démarre, avec `HÔTE ✓ clé vérifiée` puis `AUTH`. Après la saisie de la passphrase (section 4) ou directement si elle est déjà mémorisée, `AOS liaison établie`.
- [y] Redirection automatique vers le tableau de bord. La barre de navigation affiche le profil, la latence (en ms, après quelques secondes), le système, la version de Docker et celle de systemd.
- [y] `profiles.json` contient `"hostKeyFingerprint": "SHA256:…"` pour `vps-test`, identique à `ssh-keygen`.

## 2. Reconnexion avec empreinte connue

- [y] « Déconnexion », puis « Établir la liaison » sur `vps-test`.
- [y] Aucune question sur l'empreinte : `HÔTE ✓ clé vérifiée`, puis connexion et tableau de bord.
- [y] La latence se met à jour environ toutes les 30 secondes.

## 3. Empreinte différente

- [y] Se déconnecter. Dans `profiles.json`, modifier **un seul caractère** de l'empreinte de `vps-test` (garder `SHA256:` et 43 caractères). Enregistrer le fichier.
- [y] « Établir la liaison » : un écran d'alerte plein écran (pas une notification) « L'empreinte du serveur a changé. » s'affiche.
- [y] Les deux empreintes (mémorisée et reçue) sont affichées, le groupe modifié est souligné, avec « 10 groupe(s) identique(s) sur 11 ».
- [y] La télémétrie (visible après retour) ne contient aucune ligne `AUTH` : la liaison a été fermée avant l'authentification.
- [y] « Revenir aux profils » : retour à l'écran de connexion, le fichier n'a pas changé.
- [y] Recommencer, puis « Remplacer la clé mémorisée… » : le bouton « Remplacer par la clé reçue et se connecter » est **désactivé** tant que « J'ai vérifié l'empreinte sur la console de l'hébergeur » n'est pas cochée.
- [y] Cocher puis remplacer : nouvelle connexion réussie. `profiles.json` contient de nouveau l'empreinte correcte.

## 4. Passphrase : absente, incorrecte, correcte

Si une passphrase est déjà mémorisée pour `vps-test`, supprimer d'abord son entrée dans le trousseau.

- [ ] **Absente** : « Établir la liaison ». Télémétrie : `AUTH ? passphrase requise`. Le panneau « Déverrouiller la clé privée » s'affiche avec le chemin de la clé, un champ masqué et « Mémoriser dans le trousseau du système » **coché**.
- [ ] **Incorrecte, case cochée** : saisir une mauvaise passphrase. « Passphrase incorrecte. Réessayez. » s'affiche, le champ est vidé. Aucune entrée n'est créée dans le trousseau.
- [ ] **Incorrecte, case décochée** : une nouvelle séquence se lance (TCP, SSH, HÔTE), puis le panneau revient avec « Passphrase incorrecte ».
- [ ] **Correcte, case décochée** : la connexion aboutit. Se déconnecter puis se reconnecter : la passphrase est redemandée (elle n'a pas été mémorisée).
- [ ] **Correcte, case cochée** : la connexion aboutit. Une entrée `passphrase:<id>` existe dans le trousseau. Se déconnecter puis se reconnecter : plus aucune question.
- [ ] La passphrase n'apparaît dans aucun fichier : `profiles.json` ne la contient pas, ni la sortie de la console où tourne `npm run tauri dev`.
- [ ] « Annuler » sur le panneau de passphrase ramène à l'état « En attente ».

## 5. Port erroné

- [y] Connexion avec `vps-mauvais-port`.
- [y] Selon le pare-feu : soit `TCP ✗` « Connexion refusée par hôte:2223. Aucun service SSH n'écoute sur ce port. » immédiatement, soit `TCP ✗` « Aucune réponse de hôte:2223 après 10 s. » au bout de 10 secondes.
- [y] L'état affiche « Échec de la liaison » avec le même message.

## 6. Utilisateur erroné

- [y] Connexion avec `vps-mauvais-user`. Comme c'est un nouveau profil, accepter d'abord l'empreinte (section 1).
- [y] Télémétrie : `AUTH ✗` « Clé refusée par le serveur pour l'utilisateur « inexistant ». »
- [y] Message : « Authentification refusée : le serveur n'accepte pas cette clé pour cet utilisateur. »

## 7. Interruption pendant la connexion

- [y] Connexion avec `vps-mauvais-port` (cas du délai de 10 s ; à défaut, un profil vers `10.255.255.1`). Cliquer « Interrompre » pendant l'attente TCP.
- [y] Retour immédiat à « En attente », sans message d'erreur ; plus aucune étape n'arrive ensuite.
- [y] Juste après, « Établir la liaison » sur `vps-test` fonctionne normalement.
- [y] Si possible, interrompre aussi pendant la négociation (cliquer très vite après `TCP ✓`) : retour à « En attente » ; sur le VPS, `ss -tn state established '( sport = :22 )'` ne montre pas de liaison résiduelle après quelques secondes.

## 8. Déconnexion

- [y] Connecté sur `vps-test`, noter sur le VPS la liaison établie : `ss -tn state established '( sport = :22 )'`.
- [y] « Déconnexion » : retour à l'écran de connexion ; la navigation disparaît.
- [y] Sur le VPS, la liaison correspondante a disparu.
- [y] Les écrans protégés ne sont plus accessibles : Réglages reste accessible (profils locaux), le tableau de bord renvoie vers la connexion.

## 9. Contrôles complémentaires

- [y] Réglages : modifier l'hôte ou le port d'un profil qui a une empreinte affiche l'avertissement « l'empreinte mémorisée sera effacée… » ; après enregistrement, l'empreinte est bien effacée. Modifier seulement le nom la conserve.
- [y] « Définir par défaut » : un seul profil porte la mention « par défaut » ; c'est lui qui est présélectionné à l'ouverture.
- [y] « Supprimer… » passe par une confirmation ; « Annuler » ne supprime rien. Le profil de la liaison en cours ne peut pas être supprimé. Après suppression, l'entrée du trousseau du profil a disparu.
- [y] Perte de liaison : connecté, couper le réseau du PC (ou tuer la session sur le VPS) ; en moins de deux minutes, l'application revient à l'écran de connexion.
- [y] `~/.ssh/known_hosts` n'a jamais été modifié.

## Tests d'intégration automatisés (facultatif)

Ils se connectent au VPS sans passer par l'interface. PowerShell :

```powershell
$env:VPSCTL_TEST_HOST = "…"; $env:VPSCTL_TEST_USER = "…"
$env:VPSCTL_TEST_KEY = "~/.ssh/id_ed25519"
$env:VPSCTL_TEST_FINGERPRINT = "SHA256:…"
$env:VPSCTL_TEST_PASSPHRASE = "…"   # si la clé est chiffrée
cd src-tauri; cargo test --test ssh_integration -- --ignored --test-threads=1
```

Bash : mêmes variables avec `export`, puis la même commande `cargo test`.
