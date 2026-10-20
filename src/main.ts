import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { installFindShortcut } from "./findShortcut";
import { installProductionGuards } from "./productionGuards";
import { router } from "./router";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/base.css";

// Essais locaux de l'image de fond : src/local/*.css (ignoré par git), en
// développement uniquement. Absent, rien n'est chargé et le build passe.
if (import.meta.env.DEV) {
  for (const load of Object.values(import.meta.glob("./local/*.css"))) {
    void load();
  }
}

// Pas de rechargement ni de menu du moteur web en production ; en
// développement, F5 et l'inspecteur restent disponibles.
if (import.meta.env.PROD) installProductionGuards();
installFindShortcut();

createApp(App).use(createPinia()).use(router).mount("#app");
