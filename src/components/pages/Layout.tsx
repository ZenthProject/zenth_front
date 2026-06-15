import { Suspense } from "react";
import { SidebarProvider } from "@/components/ui/sidebar";
import { AppSidebar } from "../SideBar/SideBarApp";
import { Outlet, useLocation, Navigate } from "react-router-dom";
import SideBarInsetHeader from "../SideBar/SideBarInsetHeader";
import { useAutoLock } from "@/hooks/useAutoLock";
import { useGlobalMessageSync } from "@/hooks/useGlobalMessageSync";
import UpdateBanner from "@/components/modules/UpdateBanner";
import { useAuth } from "@/hooks/use-auth";
import { TopProgressBar } from "@/components/modules/TopProgressBar";

export default function Page() {
  useAutoLock();
  useGlobalMessageSync();
  const location = useLocation();
  const { isAuthenticated, isLoading } = useAuth();

  if (isLoading) return null;
  // authLogin() writes sessionStorage synchronously before React commits state,
  // so checking it here bridges the race between navigate() and the state commit.
  const hasSession = isAuthenticated || !!sessionStorage.getItem('zenth_auth');
  if (!hasSession) return <Navigate to="/login" replace />;

  return (
    <SidebarProvider className="h-screen overflow-hidden">
      <TopProgressBar />
      <AppSidebar />
      <SideBarInsetHeader>
        <UpdateBanner />
        <Suspense fallback={<div className="page-loading-bar" />}>
          <div key={location.pathname} className="page-transition h-full">
            <Outlet />
          </div>
        </Suspense>
      </SideBarInsetHeader>
    </SidebarProvider>
  );
}
