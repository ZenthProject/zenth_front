import { useLocation } from "react-router-dom";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Home } from "lucide-react";

interface BreadcrumbConfig {
  label: string;
  parent?: string;
}

const routeLabels: Record<string, BreadcrumbConfig> = {
  "/": { label: "Connexion" },
  "/login": { label: "Connexion" },
  "/register": { label: "Inscription" },
  "/keygen": { label: "Generation de cle" },
  "/dashboard": { label: "Tableau de bord" },
  "/chat": { label: "Messages", parent: "Tableau de bord" },
  "/friends": { label: "Amis", parent: "Tableau de bord" },
  "/settings": { label: "Parametres", parent: "Tableau de bord" },
  "/network": { label: "Reseau", parent: "Parametres" },
  "/search": { label: "Recherche", parent: "Tableau de bord" },
  "/relay": { label: "Relais", parent: "Reseau" },
};

export function DynamicBreadcrumb() {
  const location = useLocation();
  const currentPath = location.pathname;
  const currentConfig = routeLabels[currentPath] || { label: "Page" };

  // Build breadcrumb trail
  const breadcrumbItems: { label: string; path?: string }[] = [];

  // Add parent if exists
  if (currentConfig.parent) {
    // Find the path for the parent
    const parentEntry = Object.entries(routeLabels).find(
      ([_, config]) => config.label === currentConfig.parent
    );
    if (parentEntry) {
      breadcrumbItems.push({ label: parentEntry[1].label, path: parentEntry[0] });
    }
  }

  // Add current page (no path = current page, not clickable)
  breadcrumbItems.push({ label: currentConfig.label });

  return (
    <Breadcrumb>
      <BreadcrumbList>
        {/* Home link */}
        <BreadcrumbItem className="hidden md:block">
          <BreadcrumbLink href="/dashboard" className="flex items-center gap-1">
            <Home className="h-3.5 w-3.5" />
            <span className="sr-only">Accueil</span>
          </BreadcrumbLink>
        </BreadcrumbItem>

        {breadcrumbItems.map((item, index) => (
          <div key={index} className="flex items-center gap-2">
            <BreadcrumbSeparator className="hidden md:block" />
            <BreadcrumbItem>
              {item.path ? (
                <BreadcrumbLink href={item.path}>{item.label}</BreadcrumbLink>
              ) : (
                <BreadcrumbPage>{item.label}</BreadcrumbPage>
              )}
            </BreadcrumbItem>
          </div>
        ))}
      </BreadcrumbList>
    </Breadcrumb>
  );
}
