import { Separator } from "@/components/ui/separator";
import { SidebarInset, SidebarTrigger } from "@/components/ui/sidebar";
import { useLocation } from "react-router-dom";
import { Shield } from "lucide-react";
import { useTranslation } from "react-i18next";

export default function SideBarInsetHeader({
  children,
}: {
  children: React.ReactNode;
}) {
  const location = useLocation();
  const { t } = useTranslation();

  const routeTitles: Record<string, string> = {
    "/": t("nav.dashboard"),
    "/dashboard": t("nav.dashboard"),
    "/chat": t("nav.messages"),
    "/friends": t("nav.contacts"),
    "/settings": t("nav.settings"),
  };

  const pageTitle = routeTitles[location.pathname] || "Zenth";

  return (
    <SidebarInset>
      <header className="sticky top-0 z-10 flex shrink-0 items-center border-b border-border bg-background/80 backdrop-blur-sm"
        style={{ paddingTop: 'env(safe-area-inset-top)', minHeight: 'calc(3.5rem + env(safe-area-inset-top))' }}>
        <div className="flex items-center gap-3 px-4 flex-1">
          <SidebarTrigger className="-ml-1 h-11 w-11 text-muted-foreground hover:text-foreground hover:bg-secondary rounded-md transition-colors" />
          <Separator orientation="vertical" className="h-5" />
          <span className="text-sm font-medium text-foreground">{pageTitle}</span>
        </div>

        <div className="flex items-center gap-1 px-4">
          {/* Network status indicator */}
          <div className="flex items-center gap-1.5 px-2 py-1 rounded-md bg-secondary/50 mr-2">
            <span className="relative flex h-2 w-2">
              <span className="absolute inline-flex h-full w-full rounded-full bg-green-500 opacity-75 animate-ping" />
              <span className="relative inline-flex h-2 w-2 rounded-full bg-green-500" />
            </span>
            <Shield className="h-3.5 w-3.5 text-muted-foreground" />
          </div>

        </div>
      </header>
      <main className="flex-1 min-h-0 overflow-hidden">
        {children}
      </main>
    </SidebarInset>
  );
}
