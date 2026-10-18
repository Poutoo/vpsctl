import { createRouter, createWebHistory } from "vue-router";
import type { AppSection } from "../navigation";
import { useConnectionStore } from "../stores/connection";

declare module "vue-router" {
  interface RouteMeta {
    requiresConnection?: boolean;
    /** La vue gère ses propres marges (écran plein cadre). */
    fullBleed?: boolean;
    /** Section surlignée dans la navigation, pour un écran sans entrée propre. */
    section?: AppSection["route"];
  }
}

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", redirect: { name: "dashboard" } },
    {
      path: "/connect",
      name: "connect",
      component: () => import("../views/ConnectView.vue"),
      meta: { fullBleed: true },
    },
    {
      path: "/dashboard",
      name: "dashboard",
      component: () => import("../views/DashboardView.vue"),
      // Marges et défilement gérés par la vue (console ancrée en bas).
      meta: { requiresConnection: true, fullBleed: true },
    },
    {
      path: "/dashboard/new-service",
      name: "service-create",
      component: () => import("../views/ServiceCreateView.vue"),
      meta: { requiresConnection: true, section: "dashboard" },
    },
    {
      path: "/files/:path*",
      name: "files",
      component: () => import("../views/FilesView.vue"),
      // Marges et défilement gérés par la vue (liste virtualisée).
      meta: { requiresConnection: true, fullBleed: true },
    },
    {
      path: "/scripts",
      name: "scripts",
      component: () => import("../views/ScriptsView.vue"),
      meta: { requiresConnection: true },
    },
    {
      path: "/telemetry/:tab?",
      name: "telemetry",
      component: () => import("../views/TelemetryView.vue"),
      // Marges gérées par la vue (console sur toute la hauteur).
      meta: { requiresConnection: true, fullBleed: true },
    },
    {
      path: "/settings",
      name: "settings",
      // Accessible sans connexion : les profils sont des données locales,
      // nécessaires pour établir la première liaison.
      component: () => import("../views/SettingsView.vue"),
    },
  ],
});

// La garde s'appuie sur l'état réel de la liaison, lu auprès du backend.
router.beforeEach(async (to) => {
  const connection = useConnectionStore();
  await connection.initialize();
  if (to.meta.requiresConnection && !connection.isConnected) {
    return { name: "connect" };
  }
});
