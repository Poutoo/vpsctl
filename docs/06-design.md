# Direction artistique : Atlas céleste

Statut : maquettes validées. Les captures d'écran de référence sont dans `docs/`. Les tokens sont dans `src/styles/tokens.css`.

Ce document décrit les règles que les captures ne montrent pas. En cas de doute entre une capture et ce document, ce document l'emporte.

## Concept

Un atlas de cartographie spatiale croisé avec l'esthétique des centres de contrôle de mission des années 60-70 : cartes gravées, télémétrie, procédures. Le spatial réel et documentaire, jamais la science-fiction.

## Interdits

- Fond étoilé, particules, nébuleuses, dégradés
- Lueurs néon, glow, effets hologramme, glassmorphism
- Cockpit futuriste, hexagones, HUD
- Polices autres que les trois définies
- Emojis, mises en page entièrement centrées
- Spinner en boucle

## Couleurs

- Les composants n'utilisent que les tokens sémantiques `--color-*`, jamais les primitives ni une valeur en dur. Les icônes SVG utilisent `currentColor`.
- `--color-danger` (rouge) est réservé aux **formes** (icônes, bordures, fonds teintés) : son contraste est insuffisant pour du texte. Le texte d'erreur utilise `--color-danger-text`.
- Tout texte respecte un contraste d'au moins 4,5:1.

## Typographie

| Police | Usage |
|---|---|
| EB Garamond (`--font-title`) | Titres uniquement |
| IBM Plex Sans (`--font-ui`) | Libellés, boutons, formulaires, texte courant |
| IBM Plex Mono (`--font-mono`) | Données techniques : chemins, IDs, ports, noms d'unités, logs, étiquettes courtes en capitales espacées |

Usage des tailles : voir les commentaires de `tokens.css`. Dans l'application, un titre de page ne dépasse pas `--fs-2xl`.

Les polices sont embarquées localement dans l'application (licence OFL), jamais chargées depuis internet.

## Vocabulaire

Les libellés principaux sont les termes techniques réels : Tableau de bord, Fichiers, Scripts, Réglages, Services. Le vocabulaire spatial n'intervient qu'en couche secondaire (surtitres, statuts, étiquettes) :

| Terme | Sens |
|---|---|
| AOS / LOS | Connexion établie / perdue, ou fin d'un flux |
| ACQUISITION | Connexion en cours |
| NOMINAL | Service qui fonctionne |
| GO / NO GO | Confirmation d'une action destructrice |
| Télémétrie | Logs |
| Procédures | Scripts (surtitre uniquement) |
| Liaison | Connexion SSH |

## États

Un état n'est jamais indiqué par la couleur seule.

| État | Forme | Couleur |
|---|---|---|
| Nominal | cercle plein | `--color-success` |
| Arrêté | cercle vide | texte secondaire |
| En échec | triangle d'alerte | `--color-danger` |
| En transition | demi-disque **statique** + chrono T+ | `--color-warning` |

Correspondance des états Docker et systemd : voir `docs/03-architecture.md`.

## Composants

**Boutons** : principal (ambre plein), secondaire (contour), destructif (contour et texte rouge). Désactivé : contour **pointillé**, pas seulement grisé. En cours : chrono `T+02` à côté du libellé, jamais de spinner.

**Focus** : anneau `--focus-ring` décalé de `--focus-offset`, visible sur tous les éléments interactifs.

**GO / NO GO** : toute action destructrice. Le focus s'ouvre sur NO GO, Échap = NO GO, GO est à droite et jamais l'action par défaut. La fenêtre liste les conséquences de l'action. Pour le remplacement d'une empreinte, GO reste désactivé tant que la case de vérification n'est pas cochée.

**Notifications** : succès fermé automatiquement après 5 secondes, erreur persistante jusqu'à fermeture, 3 notifications au maximum à l'écran. Une notification ne masque jamais la console ancrée.

**Lignes de tableau** : hauteur `--row-height`, données en `--font-mono` à `--fs-data`. Sélection : fond `--color-selection` et trait d'action de 2 px à gauche. Actions en icônes, chacune avec infobulle et `aria-label`. Suppression : icône distincte (pas `×`), isolée par un séparateur. Nom tronqué : nom complet en infobulle.

**Console** : un seul composant, identique partout (télémétrie plein écran, panneau ancré, sortie de script). Légende selon la source :
- Docker et scripts : `out`, et `err` en style **neutre** (beaucoup de programmes écrivent tous leurs logs sur la sortie d'erreur).
- systemd : `inf`, `wrn`, et `ERR` en rouge (priorité ≤ 3).

Défilement automatique désactivable, désactivé automatiquement quand on remonte dans l'historique, avec un bouton « nouvelles lignes ». Bannière LOS en fin de flux. Le panneau ancré réduit la zone au-dessus de lui, il ne la recouvre pas.

**Navigation** : la section active a un trait d'action de 2 px, son numéral en ambre et son texte plein. Sur les écrans sans entrée propre (Télémétrie, Création de service), la section d'origine est surlignée. En fenêtre étroite, la navigation se réduit aux numéraux romains, chacun avec une infobulle.

**Barre de titre** : personnalisée, 32 px de haut. Zone de déplacement sur toute la barre sauf les boutons et l'indicateur AOS. Survol rouge sur le bouton Fermer. Marque (planche V) : symbole `atlas-symbole.svg` en couleur, sans retouche, sur toute la hauteur de la barre (32 × 32, marge 8, écart 4), suivi du mot « Atlas » en Garamond 17 (`--fs-brand`) ; le symbole est décoratif (texte alternatif vide), le mot le nomme. En fenêtre étroite (seuil de la navigation réduite), « CONTRÔLE VPS » disparaît pour laisser la place à l'indicateur LOS, plus long. Le symbole reste dans la barre de titre, pas dans la navigation.

## Image de fond

- Une image à faible opacité (`--bg-opacity`) occupe le fond de toute l'application.
- Les zones denses en texte (tableaux, console, listes) reposent toujours sur une surface opaque, jamais directement sur l'image.
- Le contraste est porté par les surfaces : n'importe quelle image doit pouvoir être utilisée sans retoucher le reste.
- L'utilisateur peut la remplacer par sa propre image (PNG, JPEG ou WebP), régler l'assombrissement (opacité = 1 − assombrissement, 85 % par défaut) et la garder dans ses couleurs d'origine (préférences, planche VI).
- Deux traitements à comparer en performance, notamment sous WebKitGTK : l'actuel (`--bg-blend: luminosity` + `--bg-filter`) et une image déjà désaturée avec la seule opacité.

## Animations

Principe : animer les **transitions**, jamais les états permanents. Uniquement `transform` et `opacity`. Une animation ne bloque jamais une action.

| Animation | Description | Durée |
|---|---|---|
| Acquisition du signal | Balayage radar pendant la connexion, étapes réelles affichées dans la télémétrie de liaison | durée réelle de la connexion |
| Changement d'état d'un service | Trajectoire d'allumage ou d'extinction, jouée une fois | `--dur-slow` |
| Orbite de transfert | Point qui suit l'avancement réel sur un arc entre la machine locale et le serveur, dans le sens du transfert | 200 ms linéaire entre deux mesures |
| Parallaxe | L'image de fond se décale légèrement à chaque changement de section | `--dur-slow` |
| Apparition de données | Fondu des lignes chargées | `--dur-base` |
| Fin de flux | Bannière LOS | `--dur-fast` |

Avec `prefers-reduced-motion`, les durées deviennent quasi nulles et la parallaxe est désactivée.