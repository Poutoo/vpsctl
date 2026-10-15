import { createRouter, createWebHistory } from "vue-router";
import { useConnectionStore } from "../stores/connection";

declare module "vue-router" {
  interface RouteMeta {
    requiresConnection?: boolean;
    /** La vue gère ses propres marges (écran plein cadre). */
    fullBleed?: boolean;
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
      meta: { requiresConnection: true },
    },
    {
      path: "/files/:path*",
      name: "files",
      component: () => import("../views/FilesView.vue"),
      meta: { requiresConnection: true },
    },
    {
      path: "/scripts",
      name: "scripts",
      component: () => import("../views/ScriptsView.vue"),
      meta: { requiresConnection: true },
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
