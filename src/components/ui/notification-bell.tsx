import { useState, useEffect } from "react";
import { Bell } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { useAuth } from "@/hooks/use-auth";
import { FriendService } from "@/services/friendService";
import type { PendingRequest } from "@/types/friends";

interface Notification {
  id: string;
  type: "friend_request" | "message" | "system";
  title: string;
  description?: string;
  timestamp: Date;
  read: boolean;
}

export function NotificationBell() {
  const { sessionToken, isAuthenticated } = useAuth();
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [_pendingRequests, setPendingRequests] = useState<PendingRequest[]>([]);

  useEffect(() => {
    if (!isAuthenticated || !sessionToken) return;

    const loadNotifications = async () => {
      try {
        const requests = await FriendService.listPendingRequests({ sessionToken });
        const incoming = requests.filter(r => r.direction === "incoming");
        setPendingRequests(incoming);

        // Convert pending requests to notifications
        const friendNotifs: Notification[] = incoming.map(req => ({
          id: `friend-${req.id}`,
          type: "friend_request" as const,
          title: "Demande d'ami",
          description: req.remote_pseudo || req.remote_username_hash.slice(0, 8) + "...",
          timestamp: new Date(),
          read: false,
        }));

        setNotifications(friendNotifs);
      } catch (error) {
        console.error("Failed to load notifications:", error);
      }
    };

    loadNotifications();
    const interval = setInterval(loadNotifications, 30000);
    return () => clearInterval(interval);
  }, [isAuthenticated, sessionToken]);

  const unreadCount = notifications.filter(n => !n.read).length;

  const markAllAsRead = () => {
    setNotifications(prev => prev.map(n => ({ ...n, read: true })));
  };

  if (!isAuthenticated) {
    return null;
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          className="relative h-9 w-9 text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
        >
          <Bell className="h-4 w-4" />
          {unreadCount > 0 && (
            <span
              className={cn(
                "absolute -top-0.5 -right-0.5 flex h-4 w-4 items-center justify-center",
                "rounded-full bg-destructive text-destructive-foreground",
                "text-[10px] font-medium"
              )}
            >
              {unreadCount > 9 ? "9+" : unreadCount}
            </span>
          )}
          <span className="sr-only">Notifications</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-80">
        <DropdownMenuLabel className="flex items-center justify-between">
          <span>Notifications</span>
          {unreadCount > 0 && (
            <button
              onClick={markAllAsRead}
              className="text-xs text-primary hover:underline"
            >
              Tout marquer comme lu
            </button>
          )}
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
        <ScrollArea className="h-[300px]">
          {notifications.length === 0 ? (
            <div className="p-4 text-center text-sm text-muted-foreground">
              Aucune notification
            </div>
          ) : (
            notifications.map((notification) => (
              <DropdownMenuItem
                key={notification.id}
                className={cn(
                  "flex flex-col items-start gap-1 p-3 cursor-pointer",
                  !notification.read && "bg-accent/30"
                )}
              >
                <div className="flex items-center gap-2 w-full">
                  <span
                    className={cn(
                      "h-2 w-2 rounded-full",
                      notification.read ? "bg-transparent" : "bg-primary"
                    )}
                  />
                  <span className="font-medium text-sm">{notification.title}</span>
                </div>
                {notification.description && (
                  <span className="text-xs text-muted-foreground pl-4">
                    {notification.description}
                  </span>
                )}
              </DropdownMenuItem>
            ))
          )}
        </ScrollArea>
        {notifications.length > 0 && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem className="text-center text-sm text-primary">
              Voir toutes les notifications
            </DropdownMenuItem>
          </>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
