import { createHashHistory, createRouter } from "@tanstack/react-router";

import { routeTree } from "@/app/routeTree.local-owner.gen";

export const router = createRouter({
  routeTree,
  history: createHashHistory(),
  scrollRestoration: true,
  getScrollRestorationKey: (location: { pathname: string }) =>
    location.pathname,
});
