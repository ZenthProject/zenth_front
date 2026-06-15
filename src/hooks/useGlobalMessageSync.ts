import { useEffect, useRef, useCallback } from "react";
import { useLocation } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { notifyNewMessage } from "@/services/notificationService";
import type { FriendInfo } from "@/types/friends";

interface SyncResult {
  new_messages: number;
  updated_friend_ids: number[];
}

/**
 * Sync des messages en arrière-plan sur toutes les pages sauf /chat.
 * Quand Chat.tsx est monté il gère son propre sync périodique ;
 * ce hook prend le relais sur Dashboard, Friends, Settings, etc.
 *
 * Écoute aussi relay:update pour déclencher une notification immédiate
 * quand un message arrive via le relay multi-device (pas d'attente 30s).
 */
export function useGlobalMessageSync() {
  const { sessionToken, isAuthenticated } = useAuth();
  const location = useLocation();
  const friendMapRef = useRef<Map<number, string>>(new Map());

  const refreshFriends = useCallback(async (token: string) => {
    try {
      const friends = await invoke<FriendInfo[]>("list_friends", { sessionToken: token });
      const map = new Map<number, string>();
      friends.forEach(f => map.set(f.id, f.pseudo));
      friendMapRef.current = map;
    } catch {}
  }, []);

  // Charge la liste d'amis dès la connexion
  useEffect(() => {
    if (!isAuthenticated || !sessionToken) return;
    refreshFriends(sessionToken);
  }, [isAuthenticated, sessionToken, refreshFriends]);

  useEffect(() => {
    if (!isAuthenticated || !sessionToken) return;
    if (location.pathname === "/chat") return;

    const token = sessionToken;

    const sync = async () => {
      try {
        const result = await invoke<SyncResult>("sync_messages", { sessionToken: token });
        if (result.new_messages > 0) {
          window.dispatchEvent(new CustomEvent("messages:background-new", { detail: result }));

          const names = result.updated_friend_ids
            .map(id => friendMapRef.current.get(id))
            .filter((n): n is string => Boolean(n));

          const uniqueNames = [...new Set(names)];
          if (uniqueNames.length > 0) {
            for (const name of uniqueNames) notifyNewMessage(name);
          } else {
            notifyNewMessage("Contact");
          }
        }
      } catch {}
    };

    // Réagit immédiatement aux messages relay (multi-device) sans attendre l'intervalle
    const onRelayUpdate = () => {
      refreshFriends(token);
      sync();
    };
    window.addEventListener("relay:update", onRelayUpdate);

    sync();
    const interval = setInterval(sync, 30_000);

    return () => {
      clearInterval(interval);
      window.removeEventListener("relay:update", onRelayUpdate);
    };
  }, [isAuthenticated, sessionToken, location.pathname, refreshFriends]);
}
