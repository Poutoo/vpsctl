import { createRouter, createWebHistory } from "vue-router";
import { useConnectionStore } from "../stores/connection";

declare module "vue-router" {
  interface RouteMeta {
    requiresConnection?: boolean;
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
      component: () => import("../views/SettingsView.vue"),
      meta: { requiresConnection: true },
    },
  ],
});

router.beforeEach((to) => {
  const connection = useConnectionStore();
  if (to.meta.requiresConnection && !connection.isConnected) {
    return { name: "connect" };
  }
});
