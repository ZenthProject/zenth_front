import * as React from "react";
import {
  MessageSquare,
  Users,
  Settings,
  LogOut,
} from "lucide-react";
import { Link, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
} from "@/components/ui/sidebar";
import { NavMain } from "./NavMain";
import { NavUser } from "./NavUser";
import { useAuth } from "@/hooks/use-auth";

function useNavSections() {
  const { t } = useTranslation();
  return [
    {
      label: t("nav.messaging"),
      items: [
        { title: t("nav.messages"), url: "/chat",    icon: MessageSquare },
        { title: t("nav.contacts"), url: "/friends", icon: Users },
      ],
    },
    {
      label: t("nav.system"),
      items: [
        { title: t("nav.settings"), url: "/settings", icon: Settings },
      ],
    },
  ];
}

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  const { username, sessionToken, logout } = useAuth();
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [avatar, setAvatar] = React.useState("");

  React.useEffect(() => {
    if (!sessionToken) return;
    invoke<string | null>("get_my_avatar", { sessionToken })
      .then((b64) => {
        if (b64) setAvatar(`data:image/jpeg;base64,${b64}`);
      })
      .catch(() => {});
  }, [sessionToken]);

  const user = {
    name: username || "Utilisateur",
    avatar,
  };

  return (
    <Sidebar collapsible="icon" {...props}>
      <SidebarHeader>
        <Link
          to="/chat"
          className="flex items-center gap-2.5 px-3 py-3 group"
        >
          <img
            src="/logo.svg"
            alt="Zenth"
            className="h-12 w-12 rounded-xl shrink-0 object-contain"
          />
          <span className="group-data-[collapsible=icon]:hidden font-black tracking-[0.25em] text-xl uppercase text-sidebar-foreground">
            ZENTH
          </span>
        </Link>
      </SidebarHeader>

      <SidebarContent>
        <NavMain sections={useNavSections()} />
      </SidebarContent>

      <SidebarFooter>
        <div className="flex items-center gap-1">
          <div className="flex-1 min-w-0">
            <NavUser user={user} />
          </div>
          <button
            onClick={async () => { await logout(); navigate('/login'); }}
            className="shrink-0 p-2 rounded-lg text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors"
            title={t("user_menu.logout")}
          >
            <LogOut className="h-4 w-4" />
          </button>
        </div>
      </SidebarFooter>

    </Sidebar>
  );
}
