# Application Discord d'Atlas

Réglages à faire sur le [portail des développeurs Discord](https://discord.com/developers/applications) pour la présence Discord (planche VI).

1. **New Application**, nom : `Atlas · Contrôle de mission`. Ce nom est le titre de la carte : la commande `SET_ACTIVITY` ne permet pas de le changer.
2. **Rich Presence → Art Assets** : déposer `papillon-1024.png` (1024 × 1024, taille recommandée par Discord) avec la clé `papillon`. Les clés sont en minuscules ; la carte utilise `large_image: "papillon"`. Un nouvel asset peut mettre quelques minutes à apparaître.
3. Copier l'**Application ID** (public, 17 à 20 chiffres) dans `CLIENT_ID`, `src-tauri/src/presence/mod.rs`. Tant qu'il est vide, la présence est « Indisponible ».

`papillon-1024.png` est rendu depuis `src/assets/logo/atlas-symbole.svg` (fond transparent) :

```sh
npx tauri icon src/assets/logo/atlas-symbole.svg -o <dossier temporaire> --png 1024
```
