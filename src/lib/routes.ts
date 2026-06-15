import React from "react";
import { createHashRouter } from "react-router";

export const router = createHashRouter([
  // Public routes (no sidebar)
  {
    path: "/login",
    Component: React.lazy(() => import("@/components/pages/Login")),
  },
  {
    path: "/register",
    Component: React.lazy(() => import("@/components/pages/Register")),
  },
  {
    path: "/keygen",
    Component: React.lazy(() => import("@/components/pages/KeyGen")),
  },
  // Protected routes (with sidebar layout)
  {
    path: "/",
    Component: React.lazy(() => import("@/components/pages/Layout")),
    children: [
      {
        path: "/",
        Component: React.lazy(() => import("@/components/pages/Chat")),
      },
      {
        path: "/dashboard",
        Component: React.lazy(() => import("@/components/pages/Chat")),
      },
      {
        path: "/chat",
        Component: React.lazy(() => import("@/components/pages/Chat")),
      },
{
        path: "/friends",
        Component: React.lazy(() => import("@/components/pages/Friends")),
      },
      {
        path: "/settings",
        Component: React.lazy(() => import("@/components/pages/Settings")),
      },
    ],
  },
]);
