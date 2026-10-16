# Tests manuels de l'habillage « Atlas céleste » et de la barre de titre

À dérouler sur Windows **puis** sur Linux. Chaque test indique l'action et le résultat attendu ; cocher au fur et à mesure et noter l'environnement (version de Windows ; distribution, bureau, X11 ou Wayland sous Linux).

Référence visuelle : `docs/06-design.md` (prioritaire) et les captures de `docs/design/`.

## Préparation

- Lancer l'application : `npm run tauri dev`. Elle s'ouvre en 1280 × 800.
- Image de fond pour les essais (facultatif, sections 6 et 7) : créer le dossier `src/local/` (ignoré par git), y placer une image, par exemple `fond.jpg`, et un fichier `src/local/fond.css` :

  ```css
  :root {
    --bg-image: url("./fond.jpg");
  }
  ```

  Ce dossier n'est chargé qu'en développement : `npm run tauri build` ne l'embarque jamais.
- Pour provoquer les états de la connexion :
  - **échec** : un profil vers une adresse qui ne répond pas, par exemple `192.0.2.1` (réservée à la documentation) ;
  - **première acquisition** : un profil neuf, sans empreinte mémorisée ;
  - **passphrase** : une clé chiffrée dont la passphrase n'est pas dans le trousseau ;
  - **empreinte différente** : application fermée, modifier un caractère de l'empreinte d'un profil dans `profiles.json` (Windows : `%APPDATA%\dev.poutoo.atlas\` ; Linux : `~/.config/dev.poutoo.atlas/`), comme au jalon 1.

## 1. Barre de titre : déplacement et double-clic

- [ ] Aucune barre de titre native : seule la barre « Atlas · CONTRÔLE VPS » est visible en haut.
- [ ] Glisser depuis le logo, depuis la zone vide du milieu ou depuis le texte « CONTRÔLE VPS » déplace la fenêtre.
- [ ] Glisser depuis l'indicateur AOS / LOS ou depuis un bouton de fenêtre ne déplace **pas** la fenêtre.
- [ ] Le texte de la barre n'est pas sélectionnable (pas de surlignage en glissant).
- [ ] Double-clic sur la zone vide : la fenêtre s'agrandit ; nouveau double-clic : elle revient à sa taille précédente.
- [ ] Double-clic sur l'indicateur AOS ou sur un bouton : aucun agrandissement.
- [ ] Fenêtre agrandie : glisser la barre vers le bas restaure la fenêtre et la déplace (Windows ; sous Linux, noter le comportement du gestionnaire de fenêtres).

## 2. Barre de titre : boutons

- [ ] Réduire : la fenêtre passe dans la barre des tâches ; un clic sur son icône la restaure.
- [ ] Agrandir : la fenêtre occupe l'écran, **sans déborder sous la barre des tâches**. L'icône devient « restaurer » (deux carrés), l'infobulle et le libellé deviennent « Restaurer ».
- [ ] Restaurer : retour à la taille précédente, l'icône redevient un carré.
- [ ] L'icône suit aussi un agrandissement fait autrement (double-clic, ancrage en haut de l'écran, Win+↑).
- [ ] Survol de Fermer : fond rouge, croix claire. Survol de Réduire et Agrandir : fond gris discret.
- [ ] Fermer : l'application se ferme (une liaison active est fermée avec elle, comme avant).
- [ ] Survol prolongé de chaque bouton : infobulle « Réduire », « Agrandir » ou « Restaurer », « Fermer ».
- [ ] Indicateur de liaison, selon l'état : `LOS · aucune liaison` (gris), `ACQUISITION · <profil>` (ambre, antenne), `AOS · <profil> · NN ms` (vert-de-gris, ondes), `LOS · échec de liaison` ou `LOS · liaison coupée` (cadre rouge, antenne barrée). Chaque état reste lisible sans la couleur (icône et libellé).

## 3. Redimensionnement par les bords

Windows :

- [ ] Gauche, droite, bas et les quatre coins : le curseur de redimensionnement apparaît **juste à l'extérieur** de la fenêtre (dans l'ombre), comme pour une fenêtre native.
- [ ] Bord haut : une bande fine, en haut de la barre de titre, redimensionne. Juste en dessous, la barre déplace la fenêtre.
- [ ] Impossible de descendre sous 1024 × 680.

Linux :

- [ ] Les quatre bords et les coins redimensionnent sur une bande d'environ 5 px **à l'intérieur** de la fenêtre. Noter si le curseur change bien au survol.
- [ ] Cette bande recouvre le bord haut de la barre et le bord droit du bouton Fermer : vérifier que Fermer reste cliquable sur presque toute sa surface.
- [ ] Fenêtre agrandie : pas de redimensionnement par les bords (attendu).
- [ ] Impossible de descendre sous 1024 × 680.

## 4. Ombre, bordure et ancrage

Windows :

- [ ] Ombre autour de la fenêtre ; sous Windows 11, coins arrondis. Noter la couleur de la bordure de 1 px (elle dépend du réglage « Afficher la couleur d'accentuation sur les barres de titre et bordures »).
- [ ] Glisser la barre contre le bord gauche ou droit : aperçu puis ancrage sur la moitié de l'écran ; contre le haut : agrandissement ; dans un coin : quart d'écran.
- [ ] Win+← / Win+→ / Win+↑ / Win+↓ fonctionnent.
- [ ] Alt+Espace ouvre le menu système de la fenêtre ; Alt+F4 ferme l'application.
- [ ] Limite connue : le menu d'ancrage (Snap Layouts) n'apparaît **pas** au survol du bouton agrandir. Win+Z peut l'ouvrir : noter le résultat.

Linux :

- [ ] Ombre : non prise en charge par Tauri ; noter ce qu'affiche le compositeur (aucune ombre attendue sous GNOME/Wayland).
- [ ] Glisser la barre contre un bord de l'écran : noter si le gestionnaire de fenêtres propose l'ancrage (GNOME : moitié d'écran et agrandissement en haut).
- [ ] Super+← / Super+→ / Super+↑ : noter le comportement.

## 5. Mise en page à 1280 × 800 et 1024 × 680

À 1280 × 800 (taille de lancement), puis à 1024 × 680 (réduire la fenêtre jusqu'à la butée) :

- [ ] Aucune barre de défilement horizontale, sur aucun écran.
- [ ] 1280 : navigation complète (numéral et libellé, profil en tête, « Liaison depuis HH:MM:SS » et « Couper la liaison » en pied).
- [ ] 1024 (sous 1100 px de large) : la navigation se réduit aux numéraux I à IV. Survol ou focus d'un numéral : infobulle avec le libellé. Le pied n'affiche qu'un bouton icône « Couper la liaison », avec infobulle.
- [ ] Connexion : colonne des profils à gauche, radar et état à droite, télémétrie en bas. À 1024 × 680, première acquisition et passphrase restent sur deux colonnes ; la télémétrie reste accessible en faisant défiler la colonne de droite.
- [ ] Alerte d'empreinte différente : titre, comparaison des empreintes, procédure et boutons visibles ; les groupes reçus différents sont soulignés.
- [ ] Tableau de bord : les colonnes restent lisibles, les noms tronqués affichent leur valeur complète au survol.
- [ ] Réglages : liste à gauche, formulaire à droite ; les paires Hôte/Port et Utilisateur/Clé privée restent côte à côte.

## 6. Image de fond et parallaxe

Avec `src/local/fond.css` en place :

- [ ] L'image est visible, à faible opacité, désaturée, dans les marges du contenu et autour du radar.
- [ ] Tableaux, console de télémétrie, panneaux, liste des profils, formulaires et navigation reposent sur des surfaces opaques : l'image n'apparaît jamais sous du texte dense.
- [ ] En passant d'une section à l'autre (I → II → III → IV), l'image glisse légèrement vers la gauche, sans à-coup ; dans l'autre sens, vers la droite.
- [ ] Aucun bord de l'image n'apparaît pendant ou après le glissement.
- [ ] Sans le fichier : fond uni, aucune erreur dans la console des outils de développement.
- [ ] WebKitGTK : comparer la fluidité avec le traitement actuel (`--bg-blend: luminosity` et `--bg-filter`) puis avec `--bg-blend: normal; --bg-filter: none;` sur une image déjà désaturée (06-design, « Image de fond »). Noter la différence ressentie.

## 7. Mouvement réduit

Activer la réduction des animations :
- Windows : Paramètres → Accessibilité → Effets visuels → « Effets d'animation » désactivé ;
- GNOME : Paramètres → Accessibilité → « Animations » désactivé, ou `gsettings set org.gnome.desktop.interface enable-animations false`.

Relancer l'application si le changement n'est pas pris en compte.

- [ ] Pendant une connexion, le radar affiche un secteur **fixe** au lieu d'un balayage.
- [ ] Changer de section : l'image de fond ne bouge pas.
- [ ] Les fondus (lignes des tableaux, fenêtre GO / NO GO, infobulles, bannière LOS) sont instantanés.
- [ ] Aucune action n'est retardée ou bloquée par une animation, avec ou sans réduction.
- [ ] Linux : noter si WebKitGTK suit bien le réglage du bureau (sinon, le signaler).

## 8. Navigation au clavier

- [ ] Tab depuis le haut : les boutons de fenêtre, puis l'indicateur AOS (son infobulle s'ouvre au focus), puis la navigation, puis le contenu. Un anneau ambre décalé est visible sur **chaque** élément atteint.
- [ ] Connexion : les flèches haut/bas changent de profil dans la liste ; Entrée sur « Établir la liaison » lance la connexion.
- [ ] Passphrase : le champ a le focus à l'ouverture ; Entrée déverrouille.
- [ ] Réglages : supprimer un profil non connecté ouvre GO / NO GO. Le focus est sur **NO GO** ; Échap ferme sans supprimer ; Tab atteint GO, à droite ; Entrée sur NO GO annule.
- [ ] Empreinte différente : « Remplacer la clé mémorisée… » ouvre GO / NO GO avec la case « J'ai vérifié l'empreinte reçue… ». Le focus est sur NO GO même si la case précède les boutons ; GO reste désactivé (contour pointillé) tant que la case n'est pas cochée ; Espace sur la case active GO.
- [ ] En navigation réduite (1024 px), Tab sur un numéral affiche son infobulle.

## 9. Rendu des polices

- [ ] Titres (« Liaison au serveur », « Tableau de bord », « Réglages », titres de section et de dialogue, numéraux I à IV) en EB Garamond, **graisse normale**, sans faux gras (traits fins et réguliers).
- [ ] Libellés, boutons et texte courant en IBM Plex Sans ; boutons en demi-gras.
- [ ] Données techniques (hôtes, chemins, empreintes, chronos T+, en-têtes de tableau, étiquettes en capitales espacées) en IBM Plex Mono.
- [ ] Accents, « œ », guillemets « » et points de suspension correctement rendus, sans police de secours.
- [ ] Les noms, chemins et commandes gardent leur casse dans les étiquettes en capitales (ex. `PROFIL · vps-prod-01`, `INTERROGATION · docker ps -a`).
- [ ] Outils de développement, onglet Réseau, puis Ctrl+R : toutes les requêtes vont vers `localhost` (dev) ou `tauri://` / `http://tauri.localhost` (build) ; aucune vers internet, polices comprises.

## 10. Parcours des états

Connexion :

- [ ] En attente : « EN ATTENTE » gris, télémétrie « — aucune séquence en cours — ».
- [ ] Acquisition : « ACQUISITION » ambre, balayage du radar, étapes horodatées dans la télémétrie, chrono T+ en direct ; « Interrompre » remplace « Établir la liaison », « Nouveau profil » est désactivé (pointillé) et les autres profils sont grisés.
- [ ] Échec : « LOS » rouge, message d'erreur, ligne en échec teintée et bannière « LOS · ÉCHEC DE LIAISON · T+… » au chrono de fin de tentative.
- [ ] Première acquisition : panneau « PREMIÈRE ACQUISITION · CLÉ INCONNUE », grille de 11 groupes numérotés, chaîne complète dessous.
- [ ] Passphrase : panneau « PASSPHRASE REQUISE · CLÉ CHIFFRÉE » ; une passphrase fausse affiche le triangle et « Passphrase incorrecte. Réessayez. », avec le champ en bordure rouge.
- [ ] AOS : plot du serveur sur le radar avec sa latence, juste avant l'ouverture du tableau de bord (bref).

Tableau de bord :

- [ ] Premier chargement : chaque section affiche « INTERROGATION · <commande> » avec un chrono et trois lignes vides, sans spinner.
- [ ] Rafraîchissement automatique (toutes les 5 s) : silencieux, « Actualisé à … » reste affiché et se met à jour.
- [ ] Clic sur « Actualiser » : « Interrogation · T+… » en ambre à la place de « Actualisé à … » le temps de la requête ; le bloc ne change pas de largeur.
- [ ] Rafraîchissement automatique lent (plus de 500 ms, par exemple avec une liaison ralentie) : « Interrogation · T+… » apparaît à partir de T+00.500.
- [ ] Compteurs avec formes : disque plein (nominal), cercle (arrêté), triangle (en échec), demi-disque (en transition).
- [ ] Ligne en échec : fond teinté de rouge, triangle et « EN ÉCHEC ».
- [ ] Section vide (aucun conteneur) et section en erreur (ex. utilisateur retiré du groupe `docker`) : message sur surface opaque ; l'erreur porte un triangle et un trait rouge à gauche.
