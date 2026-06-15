import { useState, useEffect, useLayoutEffect, useRef, useCallback, useMemo, lazy, Suspense } from "react";
import { listen } from "@tauri-apps/api/event";
import { useVoiceRecorder } from "@/hooks/useVoiceRecorder";
import { invoke } from "@tauri-apps/api/core";
import type { EmojiClickData } from "emoji-picker-react";
const LazyEmojiPicker = lazy(() => import("emoji-picker-react"));
import { parseMessageContent, EMOTE_MAP } from "@/lib/emotes";
import { cn } from "@/lib/utils";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { FriendService } from "@/services/friendService";
import { ChatService } from "@/services/chatService";
import { VoiceWaveform } from "@/components/modules/chat/VoiceWaveform";
import { AudioPlayer } from "@/components/modules/chat/AudioPlayer";
import { ImageLightbox } from "@/components/modules/ImageLightbox";
import { notifyNewMessage, notifyFriendAccepted } from "@/services/notificationService";
import { useAuth } from "@/hooks/use-auth";
import { useWebSocket } from "@/contexts/WebSocketContext";
import { useTranslation } from "react-i18next";
import type { FriendInfo } from "@/types/friends";
import {
  Search,
  Send,
  Lock,
  ShieldCheck,
  MoreVertical,
  Paperclip,
  MessageSquare,
  Check,
  CheckCheck,
  Wifi,
  WifiOff,
  Smile,
  Ban,
  Trash2,
  X,
  FileIcon,
  Loader2,
  ArrowLeft,
  CornerDownLeft,
  Camera,
  Mic,
  Square,
  Bookmark,
  Timer,
  ChevronDown,
  ChevronUp,
  LockKeyhole,
  LockOpen,
} from "lucide-react";

// Encode un tableau d'octets en base64 sans spread (évite RangeError pour les grands fichiers)
function bytesToBase64(bytes: number[]): string {
    const uint8 = new Uint8Array(bytes);
    let binary = '';
    const CHUNK = 8192;
    for (let i = 0; i < uint8.length; i += CHUNK) {
        binary += String.fromCharCode(...uint8.subarray(i, i + CHUNK));
    }
    return btoa(binary);
}

interface Message {
  id: string;
  content: string;
  sender: "user" | "contact";
  timestamp: Date;
  status?: "pending" | "sending" | "sent" | "delivered" | "read" | "failed";
  messageType?: string;
  fileName?: string | null;
  fileMime?: string | null;
  fileData?: string | null; // base64
  replyToId?: string | null;
}

interface Conversation {
  friend: FriendInfo;
  messages: Message[];
  lastMessage?: Message;
  unreadCount: number;
}

const isTouchDevice = typeof window !== 'undefined' && window.matchMedia('(pointer: coarse)').matches;

// Cache module-level : survit aux navigations, évite de recharger à chaque montage
const _convsCache = new Map<string, Map<number, Conversation>>();
const _friendsCache = new Map<string, FriendInfo[]>();
const _selfIdCache = new Map<string, number>();

function useBubbleRadius(): string {
  const style = localStorage.getItem("zenth_bubble_style") || "rounded";
  if (style === "square") return "rounded-none";
  if (style === "minimal") return "rounded-sm";
  return "rounded-2xl";
}

export default function Chat() {
  const { sessionToken, isAuthenticated } = useAuth();
  const { t } = useTranslation();
  const bubbleRadius = useBubbleRadius();
  const { isConnected, lastMessage, onFriendResponse } = useWebSocket();
  const [friends, setFriends] = useState<FriendInfo[]>(
    () => (sessionToken ? (_friendsCache.get(sessionToken) ?? []) : [])
  );
  const [selfFriendId, setSelfFriendId] = useState<number | null>(
    () => (sessionToken ? (_selfIdCache.get(sessionToken) ?? null) : null)
  );
  const [chatTtl, setChatTtl] = useState<number>(0);
  const [showTtlMenu, setShowTtlMenu] = useState(false);

  // Vault (Mon espace)
  const [showSelfMenu, setShowSelfMenu] = useState(false);
  const selfMenuRef = useRef<HTMLDivElement>(null);
  const [vaultStatus, setVaultStatus] = useState<{
    enabled: boolean; messages_count: number; encrypted_count: number;
  } | null>(null);
  const [vaultUnlocked, setVaultUnlocked] = useState(false);
  const [vaultDialogMode, setVaultDialogMode] = useState<
    'unlock' | 'activate' | 'change' | 'remove' | null
  >(null);
  const [vaultPassword, setVaultPassword] = useState('');
  const [vaultOldPassword, setVaultOldPassword] = useState('');
  const [vaultNewPassword, setVaultNewPassword] = useState('');
  const [vaultLoading, setVaultLoading] = useState(false);
  const [vaultError, setVaultError] = useState<string | null>(null);
  const [vaultSuccess, setVaultSuccess] = useState<string | null>(null);
  const [conversations, setConversations] = useState<Map<number, Conversation>>(
    () => (sessionToken ? (_convsCache.get(sessionToken) ?? new Map()) : new Map())
  );
  const [selectedFriendId, setSelectedFriendId] = useState<number | null>(null);
  const selectedFriendIdRef = useRef<number | null>(null);
  useEffect(() => { selectedFriendIdRef.current = selectedFriendId; }, [selectedFriendId]);
  const conversationsRef = useRef<Map<number, Conversation>>(new Map());
  useEffect(() => { conversationsRef.current = conversations; }, [conversations]);
  const [searchQuery, setSearchQuery] = useState("");
  const [messageInput, setMessageInput] = useState("");
  const [showEmotePicker, setShowEmotePicker] = useState(false);
  const [showCamera, setShowCamera] = useState(false);
  const cameraVideoRef = useRef<HTMLVideoElement>(null);
  const cameraStreamRef = useRef<MediaStream | null>(null);
  const [emojiTab, setEmojiTab] = useState<"emoji" | "emote">("emoji");
  const [showContactMenu, setShowContactMenu] = useState(false);
  const emoteWrapperRef = useRef<HTMLDivElement>(null);
  const contactMenuRef = useRef<HTMLDivElement>(null);
  const [isLoading, setIsLoading] = useState(
    () => !sessionToken || !(_friendsCache.has(sessionToken) && _convsCache.has(sessionToken))
  );
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [pendingFile, setPendingFile] = useState<{
    name: string;
    mimeType: string;
    sanitizedData: number[];
    originalSize: number;
    sanitizedSize: number;
  } | null>(null);
  const [isSanitizing, setIsSanitizing] = useState(false);
  const [pendingAudioUrl, setPendingAudioUrl] = useState<string | null>(null);
  const [deletingMessageId, setDeletingMessageId] = useState<string | null>(null);
  const [replyingTo, setReplyingTo] = useState<Message | null>(null);
  const [forwardingMessage, setForwardingMessage] = useState<Message | null>(null);
  const [forwardSearch, setForwardSearch] = useState("");
  const [longPressMenu, setLongPressMenu] = useState<{
    visible: boolean; x: number; y: number; messageId: string;
  } | null>(null);
  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const longPressMenuRef = useRef<HTMLDivElement>(null);
  const [showMessageSearch, setShowMessageSearch] = useState(false);
  const [messageSearch, setMessageSearch] = useState("");
  const [currentMatchIdx, setCurrentMatchIdx] = useState(0);
  const messageSearchRef = useRef<HTMLInputElement>(null);
  const [enterToSend, setEnterToSend] = useState(!isTouchDevice);
  const { isRecording, duration, formatDuration, analyserRef, start: startRecording, stop: stopRecording } = useVoiceRecorder();
  const cameraInputRef = useRef<HTMLInputElement>(null);

  // Convs verrouillées (PIN)
  const [unlockedFriendIds, setUnlockedFriendIds] = useState<Set<number>>(new Set());
  const [unlockedLockedFriends, setUnlockedLockedFriends] = useState<FriendInfo[]>([]);
  const pinCheckTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [lockModal, setLockModal] = useState<{
    friendId: number; step: 'enter' | 'confirm'; pin: string; confirm: string; error: string | null; loading: boolean;
  } | null>(null);
  const [removeLockModal, setRemoveLockModal] = useState<{
    friendId: number; pin: string; error: string | null; loading: boolean;
  } | null>(null);
  const [friendMenu, setFriendMenu] = useState<{
    friendId: number; x: number; y: number; isLocked: boolean;
  } | null>(null);
  const friendMenuRef = useRef<HTMLDivElement>(null);
  const [lightbox, setLightbox] = useState<{ src: string; alt: string } | null>(null);
  const [fileSendBanner, setFileSendBanner] = useState<{ message: string; done: boolean } | null>(null);

  // Load on mount and when credentials become available
  useEffect(() => {
    if (!isAuthenticated || !sessionToken) {
      setIsLoading(false);
      return;
    }

    const token = sessionToken;

    // Si les données sont déjà en cache, les états ont déjà été initialisés
    // par les lazy initializers de useState → juste un sync silencieux en arrière-plan
    if (_friendsCache.has(token) && _convsCache.has(token)) {
      ChatService.syncMessages({ sessionToken: token })
        .then(r => { r.updated_friend_ids.forEach(fid => loadMessages(fid)); })
        .catch(() => {});
      return;
    }

    setIsLoading(true);

    // initSelfSpace AVANT listFriends ET syncMessages :
    // si syncMessages tourne avant que l'entrée self-friend existe,
    // get_friend_by_hash(our_hash) retourne None → les messages sont sautés
    // et last_message_sync avance → l'historique "Mon espace" est définitivement perdu.
    const loadAll = async () => {
      const selfId = await FriendService.initSelfSpace({ sessionToken: token }).catch(() => null);
      if (selfId !== null) {
        _selfIdCache.set(token, selfId);
        setSelfFriendId(selfId);
      }

      // Sync APRÈS initSelfSpace : l'entrée self-friend existe, les messages
      // auto-envoyés seront correctement reconnus et stockés.
      ChatService.syncMessages({ sessionToken: token }).catch(() => {});

      const friendsList = await FriendService.listFriends({ sessionToken: token });
      _friendsCache.set(token, friendsList);

      // Fallback : si initSelfSpace a échoué, on identifie "Mon espace" par sa clé publique
      if (selfId === null && friendsList.length > 0) {
        const myHash = await FriendService.getMyPublicKey({ sessionToken: token }).catch(() => null);
        if (myHash) {
          const selfEntry = friendsList.find(f => f.username_hash === myHash);
          if (selfEntry) {
            _selfIdCache.set(token, selfEntry.id);
            setSelfFriendId(selfEntry.id);
          }
        }
      }

      setFriends(friendsList);
      setIsLoading(false);

      // Initialise toutes les convs vides immédiatement → sidebar visible sans attendre
      const convMap = new Map<number, Conversation>(
        friendsList.map(f => [f.id, { friend: f, messages: [], unreadCount: 0 }])
      );
      setConversations(new Map(convMap));

      // Charge chaque conv progressivement et met à jour l'état au fil de l'eau
      await Promise.all(friendsList.map(async (friend) => {
        try {
          const messages = await ChatService.getMessages({
            sessionToken: token,
            friendId: friend.id,
            limit: 50,
          });

          const uiMessages: Message[] = messages.map(msg => ({
            id: msg.message_id,
            content: msg.content,
            sender: msg.is_outgoing ? "user" as const : "contact" as const,
            timestamp: new Date(msg.timestamp * 1000),
            status: msg.status as Message["status"],
            messageType: msg.message_type,
            fileName: msg.file_name,
            fileMime: msg.file_mime,
            fileData: msg.file_data,
            replyToId: msg.reply_to_id ?? undefined,
          }));
          uiMessages.sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime());

          convMap.set(friend.id, {
            friend,
            messages: uiMessages,
            lastMessage: uiMessages[uiMessages.length - 1],
            unreadCount: 0,
          });
          setConversations(new Map(convMap));
        } catch {
          // conv déjà initialisée vide, rien à faire
        }
      }));

      _convsCache.set(token, convMap);
    };

    loadAll().catch((error) => {
      console.error("Initial load failed:", error);
      setIsLoading(false);
    });
  }, [isAuthenticated, sessionToken]);

  // Load messages + TTL quand on change de conversation
  useEffect(() => {
    if (selectedFriendId && sessionToken) {
      loadMessages(selectedFriendId);
      ChatService.syncMessages({ sessionToken })
        .then(result => { result.updated_friend_ids.forEach(fid => loadMessages(fid)); })
        .catch(() => {});
      invoke<number>("get_chat_ttl", { sessionToken, friendId: selectedFriendId })
        .then(ttl => setChatTtl(ttl))
        .catch(() => setChatTtl(0));

      // Charge le statut vault si c'est Mon espace
      if (selectedFriendId === selfFriendId) {
        invoke<{ enabled: boolean; messages_count: number; encrypted_count: number }>(
          "get_vault_status", { sessionToken }
        ).then(s => setVaultStatus(s)).catch(() => {});
      }
    }
  }, [selectedFriendId, sessionToken]);

  // Handle incoming WebSocket messages
  useEffect(() => {
    if (lastMessage && lastMessage.type === 'message' && sessionToken) {
      ChatService.syncMessages({ sessionToken }).then(result => {
        if (result.new_messages > 0) {
          // Recharger toutes les convs mises à jour - loadMessages déclenchera les notifs
          result.updated_friend_ids.forEach(fid => loadMessages(fid));
        }
      }).catch(error => {
        console.error("Failed to sync messages after push:", error);
      });
    }
  }, [lastMessage, sessionToken]);

  // Handle friend response notifications (when someone accepts our request)
  useEffect(() => {
    const unsubscribe = onFriendResponse(async (_notification, accepted) => {
      if (accepted && sessionToken) {
        try {
          const result = await FriendService.syncFriendResponses({ sessionToken });
          if (result.new_accepted > 0) {
            const friendsList = await FriendService.listFriends({ sessionToken });
            setFriends(friendsList);
            notifyFriendAccepted("Un contact");
          }
        } catch (error) {
          console.error('[Chat] Failed to sync after friend acceptance:', error);
        }
      }
    });

    return unsubscribe;
  }, [onFriendResponse, sessionToken]);

  // Sync périodique : 10s sans WS, 30s avec WS (filet de sécurité)
  useEffect(() => {
    if (!isAuthenticated || !sessionToken) return;

    const interval = isConnected ? 30000 : 10000;
    const syncInterval = setInterval(async () => {
      try {
        const result = await ChatService.syncMessages({ sessionToken });
        if (result.new_messages > 0) {
          result.updated_friend_ids.forEach(fid => loadMessages(fid));
        }
      } catch {
        // silencieux
      }
    }, interval);

    return () => clearInterval(syncInterval);
  }, [isAuthenticated, sessionToken, selectedFriendId, isConnected]);

  useLayoutEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "instant" as ScrollBehavior });
  }, [conversations, selectedFriendId]);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 120)}px`;
    }
  }, [messageInput]);

  const loadMessages = useCallback(async (friendId: number) => {
    if (!sessionToken) return;

    try {
      const serverMessages = await ChatService.getMessages({
        sessionToken,
        friendId,
        limit: 100,
      });

      const uiMessages: Message[] = serverMessages.map(msg => ({
        id: msg.message_id,
        content: msg.content,
        sender: msg.is_outgoing ? "user" as const : "contact" as const,
        timestamp: new Date(msg.timestamp * 1000),
        status: msg.status as Message["status"],
        messageType: msg.message_type,
        fileName: msg.file_name,
        fileMime: msg.file_mime,
        fileData: msg.file_data,
      }));

      uiMessages.sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime());

      // Calcul du diff AVANT setState pour déclencher notif sans effet de bord dans l'updater
      const isViewing = selectedFriendIdRef.current === friendId;
      const prevConv = conversationsRef.current.get(friendId);
      const existingIds = prevConv ? new Set(prevConv.messages.map(m => m.id)) : new Set<string>();
      const newIncoming = uiMessages.filter(m => m.sender === "contact" && !existingIds.has(m.id));

      setConversations(prev => {
        const newMap = new Map(prev);
        const conv = newMap.get(friendId);
        if (conv) {
          const dbIds = new Set(uiMessages.map(m => m.id));
          const pending = conv.messages.filter(
            m => m.status === "sending" && !dbIds.has(m.id)
          );
          const merged = [...uiMessages, ...pending];
          merged.sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime());

          newMap.set(friendId, {
            ...conv,
            messages: merged,
            lastMessage: merged[merged.length - 1],
            unreadCount: isViewing ? 0 : conv.unreadCount + newIncoming.length,
          });
        }
        return newMap;
      });

      // Notification OS si nouveaux messages entrants non vus
      if (!isViewing && newIncoming.length > 0 && prevConv) {
        notifyNewMessage(prevConv.friend.pseudo);
      }
    } catch (error) {
      console.error("Failed to load messages:", error);
    }
  }, [sessionToken]);

  // Réagit aux mises à jour relay (multi-device sync) : sync complet + rechargement
  useEffect(() => {
    const handler = () => {
      if (!sessionToken) return;
      ChatService.syncMessages({ sessionToken })
        .then(result => { result.updated_friend_ids.forEach(fid => loadMessages(fid)); })
        .catch(() => {});
      // Recharge aussi la conv sélectionnée (messages relay déjà en DB)
      if (selectedFriendIdRef.current) loadMessages(selectedFriendIdRef.current);
    };
    window.addEventListener("relay:update", handler);
    return () => window.removeEventListener("relay:update", handler);
  }, [sessionToken, loadMessages]);

  // Ferme le menu contact au clic extérieur
  useEffect(() => {
    if (!friendMenu) return;
    const handler = (e: MouseEvent) => {
      if (friendMenuRef.current && !friendMenuRef.current.contains(e.target as Node)) {
        setFriendMenu(null);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [friendMenu]);

  // Vérifie si le searchQuery est un PIN de conv verrouillée (debounce 400ms)
  useEffect(() => {
    if (pinCheckTimerRef.current) clearTimeout(pinCheckTimerRef.current);
    if (searchQuery.length < 4 || !sessionToken) return;

    pinCheckTimerRef.current = setTimeout(async () => {
      try {
        const matches = await invoke<FriendInfo[]>('check_conversation_pin', {
          sessionToken, pin: searchQuery,
        });
        if (matches.length === 0) return;

        setUnlockedLockedFriends(prev => {
          const existing = new Set(prev.map(f => f.id));
          return [...prev, ...matches.filter(f => !existing.has(f.id))];
        });
        setUnlockedFriendIds(prev => {
          const s = new Set(prev);
          matches.forEach(f => s.add(f.id));
          return s;
        });
        // Initialise la conv et charge les messages pour chaque ami déverrouillé
        matches.forEach(f => {
          setConversations(prev => {
            if (prev.has(f.id)) return prev;
            const m = new Map(prev);
            m.set(f.id, { friend: f, messages: [], unreadCount: 0 });
            return m;
          });
          loadMessages(f.id);
        });
      } catch {}
    }, 400);
  }, [searchQuery, sessionToken, loadMessages]);

  const handleRemoveLock = async () => {
    if (!removeLockModal || !sessionToken) return;
    setRemoveLockModal(prev => prev ? { ...prev, loading: true, error: null } : null);
    try {
      await invoke('remove_conversation_lock', {
        sessionToken, friendId: removeLockModal.friendId, pin: removeLockModal.pin,
      });
      // Passe la conv de "verrouillée déverrouillée" à "normale"
      const friend = unlockedLockedFriends.find(f => f.id === removeLockModal.friendId);
      if (friend) {
        setFriends(prev => [...prev, friend].sort((a, b) => a.pseudo.localeCompare(b.pseudo)));
      }
      setUnlockedLockedFriends(prev => prev.filter(f => f.id !== removeLockModal.friendId));
      setUnlockedFriendIds(prev => { const s = new Set(prev); s.delete(removeLockModal.friendId); return s; });
      setRemoveLockModal(null);
    } catch (err) {
      setRemoveLockModal(prev => prev ? { ...prev, error: String(err), loading: false } : null);
    }
  };

  const getInitials = (name: string) => {
    return name
      .split(" ")
      .map((n) => n[0])
      .join("")
      .toUpperCase()
      .slice(0, 2);
  };

  const formatTime = (date: Date) => {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  };

  const formatLastSeen = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const days = Math.floor(diff / (1000 * 60 * 60 * 24));

    if (days === 0) return "Aujourd'hui";
    if (days === 1) return "Hier";
    if (days < 7) return `Il y a ${days} jours`;
    return date.toLocaleDateString();
  };

  // "Mon espace" est toujours épinglé en premier, les autres sont filtrés par la recherche
  const selfFriend = friends.find(f => f.id === selfFriendId) ?? null;
  const otherFriends = friends.filter(f => f.id !== selfFriendId);
  const getLastTs = (id: number) =>
    conversations.get(id)?.lastMessage?.timestamp.getTime() ?? 0;
  const filteredOthers = otherFriends
    .filter(f => f.pseudo.toLowerCase().includes(searchQuery.toLowerCase()))
    .sort((a, b) => getLastTs(b.id) - getLastTs(a.id));
  // Les convs déverrouillées par PIN sont ajoutées à la suite (filtrées aussi par texte)
  const filteredUnlocked = unlockedLockedFriends.filter(f =>
    f.pseudo.toLowerCase().includes(searchQuery.toLowerCase())
  );
  const filteredFriends = selfFriend
    ? [selfFriend, ...filteredOthers, ...filteredUnlocked]
    : [...filteredOthers, ...filteredUnlocked];

  const selectedFriend = selectedFriendId
    ? friends.find(f => f.id === selectedFriendId) ?? null
    : null;

  const selectedConversation = selectedFriendId
    ? conversations.get(selectedFriendId)
      ?? (selectedFriend ? { friend: selectedFriend, messages: [], unreadCount: 0 } : null)
    : null;

  const handleSendMessage = async () => {

    if ((!messageInput.trim() && !pendingFile) || !selectedFriendId || !sessionToken) {
      return;
    }

    if (selectedFriendId === selfFriendId && vaultStatus?.enabled && !vaultUnlocked) {
      return;
    }

    const content = messageInput.trim();
    const replyToIdCapture = replyingTo?.id ?? undefined;
    const tempId = crypto.randomUUID();

    const newMessage: Message = {
      id: tempId,
      content: pendingFile ? pendingFile.name : content,
      sender: "user",
      timestamp: new Date(),
      status: "sending",
      messageType: pendingFile ? (
        pendingFile.mimeType.startsWith("image/") ? "image" :
        pendingFile.mimeType.startsWith("audio/") ? "audio" :
        pendingFile.mimeType.startsWith("video/") ? "video" :
        pendingFile.mimeType === "application/pdf" ? "pdf" : "file"
      ) : "text",
      fileName: pendingFile?.name ?? null,
      fileMime: pendingFile?.mimeType ?? null,
      fileData: pendingFile
        ? bytesToBase64(pendingFile.sanitizedData)
        : null,
      replyToId: replyToIdCapture,
    };

    setConversations(prev => {
      const newMap = new Map(prev);
      const conv = newMap.get(selectedFriendId);
      if (conv) {
        newMap.set(selectedFriendId, {
          ...conv,
          messages: [...conv.messages, newMessage],
          lastMessage: newMessage,
        });
      }
      return newMap;
    });

    setMessageInput("");
    setReplyingTo(null);
    const fileToSend = pendingFile;
    setPendingFile(null);
    if (pendingAudioUrl) { URL.revokeObjectURL(pendingAudioUrl); setPendingAudioUrl(null); }

    try {
      const sentMessage = await ChatService.sendMessage({
        sessionToken,
        friendId: selectedFriendId,
        content: content || (fileToSend?.name ?? ""),  // content vide → nom du fichier
        fileData: fileToSend?.sanitizedData,
        fileName: fileToSend?.name,
        fileMime: fileToSend?.mimeType,
        replyToId: replyToIdCapture,
      });

      // Update with real message ID and status
      setConversations(prev => {
        const newMap = new Map(prev);
        const conv = newMap.get(selectedFriendId);
        if (conv) {
          const updatedMessages = conv.messages.map(msg =>
            msg.id === tempId
              ? {
                  ...msg,
                  id: sentMessage.message_id,
                  status: sentMessage.status as Message["status"],
                }
              : msg
          );
          newMap.set(selectedFriendId, { ...conv, messages: updatedMessages });
        }
        return newMap;
      });

      // Sync après envoi pour afficher les messages reçus pendant la rédaction
      ChatService.syncMessages({ sessionToken })
        .then(result => { result.updated_friend_ids.forEach(fid => loadMessages(fid)); })
        .catch(() => {});
    } catch (error) {
      console.error("[Chat] Failed to send message:", error);
      console.error("[Chat] Error type:", typeof error);
      console.error("[Chat] Error details:", JSON.stringify(error, null, 2));
      // Mark message as failed
      setConversations(prev => {
        const newMap = new Map(prev);
        const conv = newMap.get(selectedFriendId);
        if (conv) {
          const updatedMessages = conv.messages.map(msg =>
            msg.id === tempId ? { ...msg, status: "failed" as const } : msg
          );
          newMap.set(selectedFriendId, { ...conv, messages: updatedMessages });
        }
        return newMap;
      });
    }
  };

  // Close emote picker on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (emoteWrapperRef.current && !emoteWrapperRef.current.contains(e.target as Node)) {
        setShowEmotePicker(false);
      }
    };
    if (showEmotePicker) document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [showEmotePicker]);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (contactMenuRef.current && !contactMenuRef.current.contains(e.target as Node)) {
        setShowContactMenu(false);
      }
    };
    if (showContactMenu) document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [showContactMenu]);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (selfMenuRef.current && !selfMenuRef.current.contains(e.target as Node)) {
        setShowSelfMenu(false);
      }
    };
    if (showSelfMenu) document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [showSelfMenu]);

  useEffect(() => {
    if (showMessageSearch) messageSearchRef.current?.focus();
    else { setMessageSearch(""); setCurrentMatchIdx(0); }
  }, [showMessageSearch]);

  useEffect(() => {
    setCurrentMatchIdx(0);
  }, [messageSearch]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "f" && selectedFriendId) {
        e.preventDefault();
        setShowMessageSearch(true);
      }
      if (e.key === "Escape" && showMessageSearch) {
        setShowMessageSearch(false);
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [selectedFriendId, showMessageSearch]);

  useEffect(() => {
    if (!longPressMenu?.visible) return;
    const handler = (e: MouseEvent | TouchEvent) => {
      if (longPressMenuRef.current && !longPressMenuRef.current.contains(e.target as Node)) {
        setLongPressMenu(null);
      }
    };
    document.addEventListener("mousedown", handler);
    document.addEventListener("touchstart", handler);
    return () => {
      document.removeEventListener("mousedown", handler);
      document.removeEventListener("touchstart", handler);
    };
  }, [longPressMenu?.visible]);

  const openCamera = async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: "environment" },
      });
      cameraStreamRef.current = stream;
      setShowCamera(true);
      // Attache le stream après le rendu
      setTimeout(() => {
        if (cameraVideoRef.current) {
          cameraVideoRef.current.srcObject = stream;
          cameraVideoRef.current.play();
        }
      }, 50);
    } catch {
      // Fallback sur le sélecteur natif
      cameraInputRef.current?.click();
    }
  };

  const capturePhoto = async () => {
    const video = cameraVideoRef.current;
    if (!video) return;
    const canvas = document.createElement("canvas");
    canvas.width  = video.videoWidth;
    canvas.height = video.videoHeight;
    canvas.getContext("2d")?.drawImage(video, 0, 0);
    canvas.toBlob(async (blob) => {
      if (!blob) return;
      const file = new File([blob], `photo_${Date.now()}.jpg`, { type: "image/jpeg" });
      closeCamera();
      await processFile(file);
    }, "image/jpeg", 0.9);
  };

  const closeCamera = () => {
    cameraStreamRef.current?.getTracks().forEach(t => t.stop());
    cameraStreamRef.current = null;
    setShowCamera(false);
  };

  const handleDeleteMessage = async (messageId: string) => {
    if (!sessionToken || !selectedFriendId) return;
    try {
      await invoke("delete_message_secure", { sessionToken, messageId });
      setConversations(prev => {
        const next = new Map(prev);
        const conv = next.get(selectedFriendId);
        if (conv) next.set(selectedFriendId, { ...conv, messages: conv.messages.filter(m => m.id !== messageId) });
        return next;
      });
    } catch (e) {
      console.error("Secure delete failed:", e);
    }
    setDeletingMessageId(null);
  };

  const startLongPress = (messageId: string, clientX: number, clientY: number) => {
    if (longPressTimerRef.current) clearTimeout(longPressTimerRef.current);
    longPressTimerRef.current = setTimeout(() => {
      const x = Math.min(clientX, window.innerWidth - 160);
      const y = Math.max(clientY - 90, 8);
      setLongPressMenu({ visible: true, x, y, messageId });
    }, 450);
  };

  const cancelLongPress = () => {
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
  };

  const handleReplyAction = () => {
    if (!longPressMenu || !selectedConversation) return;
    const msg = selectedConversation.messages.find(m => m.id === longPressMenu.messageId);
    if (msg) setReplyingTo(msg);
    setLongPressMenu(null);
  };

  const handleForwardAction = () => {
    if (!longPressMenu || !selectedConversation) return;
    const msg = selectedConversation.messages.find(m => m.id === longPressMenu.messageId);
    if (msg) { setForwardingMessage(msg); setForwardSearch(""); }
    setLongPressMenu(null);
  };

  const handleForwardTo = async (targetFriendId: number) => {
    if (!forwardingMessage || !sessionToken) return;
    setForwardingMessage(null);
    try {
      await ChatService.sendMessage({
        sessionToken,
        friendId: targetFriendId,
        content: forwardingMessage.content,
        fileData: forwardingMessage.fileData
          ? Array.from(Uint8Array.from(atob(forwardingMessage.fileData), c => c.charCodeAt(0)))
          : undefined,
        fileName: forwardingMessage.fileName ?? undefined,
        fileMime: forwardingMessage.fileMime ?? undefined,
      });
      // Recharge la conv cible si elle est ouverte
      loadMessages(targetFriendId);
    } catch (e) {
      console.error("[Forward] failed:", e);
    }
  };

  const handleDeleteAction = () => {
    if (!longPressMenu) return;
    setDeletingMessageId(longPressMenu.messageId);
    setLongPressMenu(null);
  };

  const handleBlockContact = async () => {
    if (!selectedFriendId || !sessionToken) return;
    setShowContactMenu(false);
    try {
      await FriendService.blockFriend({ sessionToken, friendId: selectedFriendId });
      setSelectedFriendId(null);
      const friendsList = await FriendService.listFriends({ sessionToken });
      setFriends(friendsList);
    } catch (error) {
      console.error("Failed to block contact:", error);
    }
  };

  const handleRemoveContact = async () => {
    if (!selectedFriendId || !sessionToken) return;
    setShowContactMenu(false);
    try {
      await FriendService.removeFriend({ sessionToken, friendId: selectedFriendId });
      setSelectedFriendId(null);
      const friendsList = await FriendService.listFriends({ sessionToken });
      setFriends(friendsList);
      setConversations(prev => {
        const next = new Map(prev);
        next.delete(selectedFriendId);
        return next;
      });
    } catch (error) {
      console.error("Failed to remove contact:", error);
    }
  };

  const insertEmote = (hash: string) => {
    setMessageInput(prev => prev + `[IMAGE]:${hash}`);
    setShowEmotePicker(false);
    textareaRef.current?.focus();
  };

  const insertEmoji = (emojiData: EmojiClickData) => {
    setMessageInput(prev => prev + emojiData.emoji);
    textareaRef.current?.focus();
  };

  const processFile = async (file: File) => {
    setIsSanitizing(true);
    try {
      const arrayBuffer = await file.arrayBuffer();
      const fileData = Array.from(new Uint8Array(arrayBuffer));

      const result = await invoke<{
        success: boolean;
        message: string;
        sanitized_data: number[] | null;
        original_size: number;
        sanitized_size: number;
      }>("sanitize_file", { filePath: file.name, fileData });

      if (!result.success || !result.sanitized_data) {
        setPendingFile({ name: file.name, mimeType: file.type, sanitizedData: fileData, originalSize: result.original_size, sanitizedSize: result.original_size });
      } else {
        setPendingFile({ name: file.name, mimeType: file.type, sanitizedData: result.sanitized_data, originalSize: result.original_size, sanitizedSize: result.sanitized_size });
      }
    } catch {
      setPendingFile(null);
    } finally {
      setIsSanitizing(false);
    }
  };

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const raw = e.target.files?.[0];
    e.target.value = "";
    if (!raw) return;

    // Certains appareils Android retournent un mimeType vide pour les photos caméra
    const ext = raw.name.split(".").pop()?.toLowerCase() ?? "";
    const extMime: Record<string, string> = {
      jpg: "image/jpeg", jpeg: "image/jpeg", png: "image/png",
      gif: "image/gif", webp: "image/webp", heic: "image/heic",
      mp4: "video/mp4", mov: "video/quicktime",
    };
    const mimeType = raw.type || extMime[ext] || "application/octet-stream";
    const file = raw.type ? raw : new File([raw], raw.name, { type: mimeType });
    await processFile(file);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && enterToSend) {
      e.preventDefault();
      handleSendMessage();
    }
  };

  const TTL_OPTIONS = [
    { labelKey: "chat.retention_never", hours: 0 },
    { labelKey: "chat.retention_6h",    hours: 6 },
    { labelKey: "chat.retention_24h",   hours: 24 },
    { labelKey: "chat.retention_48h",   hours: 48 },
    { labelKey: "chat.retention_7d",    hours: 168 },
    { labelKey: "chat.retention_30d",   hours: 720 },
  ] as const;

  const ttlLabel = t(TTL_OPTIONS.find(o => o.hours === chatTtl)?.labelKey ?? "chat.retention_never");

  // IDs des messages qui contiennent le terme recherché, dans l'ordre d'affichage
  const searchMatchIds = useMemo(() => {
    const q = messageSearch.trim().toLowerCase();
    if (!q || !selectedConversation) return [] as string[];
    return selectedConversation.messages
      .filter(m => m.content.toLowerCase().includes(q))
      .map(m => m.id);
  }, [messageSearch, selectedConversation]);

  // Scroll vers le match courant
  useEffect(() => {
    if (searchMatchIds.length === 0) return;
    const id = searchMatchIds[currentMatchIdx];
    document.getElementById(`msg-${id}`)?.scrollIntoView({ behavior: "smooth", block: "center" });
  }, [currentMatchIdx, searchMatchIds]);

  useEffect(() => {
    let unlistenStarted: (() => void) | null = null;
    let unlistenComplete: (() => void) | null = null;
    let clearTimer: ReturnType<typeof setTimeout> | null = null;

    listen<{ transfer_id: number[]; filename: string; total_chunks: number }>("file-send-started", ({ payload }) => {
      if (clearTimer) clearTimeout(clearTimer);
      setFileSendBanner({ message: t("chat.file_send_started", { filename: payload.filename }), done: false });
    }).then(fn => { unlistenStarted = fn; });

    listen<{ transfer_id: number[]; filename: string }>("file-send-complete", ({ payload }) => {
      if (clearTimer) clearTimeout(clearTimer);
      setFileSendBanner({ message: t("chat.file_send_complete", { filename: payload.filename }), done: true });
      clearTimer = setTimeout(() => setFileSendBanner(null), 4000);
    }).then(fn => { unlistenComplete = fn; });

    return () => {
      unlistenStarted?.();
      unlistenComplete?.();
      if (clearTimer) clearTimeout(clearTimer);
    };
  }, [t]);

  const highlightText = (text: string, isCurrentMatch: boolean) => {
    const q = messageSearch.trim();
    if (!q) return <>{text}</>;
    const escaped = q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const parts = text.split(new RegExp(`(${escaped})`, "gi"));
    return (
      <>
        {parts.map((part, i) =>
          part.toLowerCase() === q.toLowerCase()
            ? <mark key={i} className={cn(
                "rounded-sm px-0.5",
                isCurrentMatch
                  ? "bg-primary text-primary-foreground"
                  : "bg-primary/30 text-foreground"
              )}>{part}</mark>
            : part
        )}
      </>
    );
  };

  const handleSetTtl = async (hours: number) => {
    if (!selectedFriendId || !sessionToken) return;
    await invoke("set_chat_ttl", { sessionToken, friendId: selectedFriendId, ttlHours: hours });
    setChatTtl(hours);
    setShowTtlMenu(false);
    setShowContactMenu(false);
  };

  // Handlers vault
  // Déverrouille le vault et recharge les messages - utilisé par l'overlay ET le menu.
  const doVaultUnlock = async (password: string) => {
    if (!sessionToken) return;
    setVaultLoading(true);
    setVaultError(null);
    try {
      const ok = await invoke<boolean>("unlock_vault", { sessionToken, password });
      if (!ok) { setVaultError(t("chat.vault_wrong_password")); return; }
      // Charger les messages déchiffrés AVANT de masquer l'overlay : les deux setState
      // se retrouvent dans le même batch React → overlay disparaît en même temps que les
      // messages déchiffrés apparaissent, sans fenêtre de bulles vides entre les deux.
      if (selfFriendId) await loadMessages(selfFriendId);
      setVaultUnlocked(true);
      setVaultPassword('');
      setVaultDialogMode(null);
    } catch { setVaultError(t("chat.vault_wrong_password")); }
    finally { setVaultLoading(false); }
  };

  const handleVaultAction = async () => {
    if (!sessionToken) return;
    setVaultLoading(true);
    setVaultError(null);
    setVaultSuccess(null);

    try {
      if (vaultDialogMode === 'unlock') {
        await doVaultUnlock(vaultPassword);
        return;

      } else if (vaultDialogMode === 'activate') {
        const count = await invoke<number>("set_vault_password", {
          sessionToken, newPassword: vaultPassword, oldPassword: null,
        });
        // Auto-déverrouille après activation - évite de saisir le mot de passe deux fois.
        await invoke<boolean>("unlock_vault", { sessionToken, password: vaultPassword });
        setVaultUnlocked(true);
        setVaultSuccess(t("chat.vault_success", { count }));
        const s = await invoke<typeof vaultStatus>("get_vault_status", { sessionToken });
        setVaultStatus(s);
        setVaultDialogMode(null);
        if (selfFriendId) loadMessages(selfFriendId);

      } else if (vaultDialogMode === 'change') {
        const count = await invoke<number>("set_vault_password", {
          sessionToken, newPassword: vaultNewPassword, oldPassword: vaultOldPassword,
        });
        // Met à jour la clé en cache avec le nouveau mot de passe.
        await invoke<boolean>("unlock_vault", { sessionToken, password: vaultNewPassword });
        setVaultUnlocked(true);
        setVaultSuccess(t("chat.vault_success", { count }));
        const s = await invoke<typeof vaultStatus>("get_vault_status", { sessionToken });
        setVaultStatus(s);
        setVaultDialogMode(null);
        if (selfFriendId) loadMessages(selfFriendId);

      } else if (vaultDialogMode === 'remove') {
        await invoke<number>("remove_vault_password", { sessionToken, currentPassword: vaultPassword });
        // Fetch updated status BEFORE updating local state so all setStates below
        // are batched in a single render where enabled=false → no overlay flash.
        const s = await invoke<typeof vaultStatus>("get_vault_status", { sessionToken });
        setVaultStatus(s);
        setVaultUnlocked(false);
        setVaultSuccess(t("chat.vault_removed"));
        setVaultDialogMode(null);
        if (selfFriendId) loadMessages(selfFriendId);
      }
    } catch (e) {
      setVaultError(String(e).includes("incorrect") ? t("chat.vault_wrong_password") : String(e));
    } finally {
      setVaultLoading(false);
      setVaultPassword('');
      setVaultOldPassword('');
      setVaultNewPassword('');
    }
  };

  const SelfSpaceMenu = () => (
    <div ref={selfMenuRef} className="relative">
      <button
        onClick={() => { setShowSelfMenu(v => !v); setVaultDialogMode(null); setVaultError(null); setVaultSuccess(null); }}
        className="h-8 w-8 flex items-center justify-center rounded-md text-muted-foreground hover:text-foreground hover:bg-secondary transition-colors"
        title={t("chat.vault_title")}
      >
        <Lock className="w-4 h-4" />
      </button>

      {showSelfMenu && (
        <div className="absolute right-0 top-full mt-1 w-64 bg-card border border-border rounded-lg shadow-lg z-50 overflow-hidden">
          {/* Header */}
          <div className="px-3 py-2.5 border-b border-border bg-muted/30">
            <p className="text-xs font-semibold uppercase tracking-widest text-muted-foreground">
              {t("chat.vault_title")}
            </p>
            {vaultStatus && (
              <p className="text-xs text-muted-foreground mt-0.5">
                {t("chat.vault_messages", { count: vaultStatus.messages_count })}
                {vaultStatus.enabled && ` · ${t("chat.vault_encrypted", { count: vaultStatus.encrypted_count })}`}
              </p>
            )}
          </div>

          {/* Statut */}
          <div className="px-3 py-2 flex items-center justify-between">
            <span className="text-sm text-foreground">
              {vaultStatus?.enabled
                ? (vaultUnlocked ? t("chat.vault_status_unlocked") : t("chat.vault_status_locked"))
                : t("chat.vault_status_off")}
            </span>
            <span className={cn(
              "text-xs px-1.5 py-0.5 rounded-full font-medium",
              vaultStatus?.enabled
                ? vaultUnlocked ? "bg-success/10 text-success" : "bg-amber-500/10 text-amber-500"
                : "bg-muted text-muted-foreground"
            )}>
              {vaultStatus?.enabled ? (vaultUnlocked ? "●" : "-") : "○"}
            </span>
          </div>

          {/* Feedback */}
          {vaultError && <p className="px-3 pb-2 text-xs text-destructive">{vaultError}</p>}
          {vaultSuccess && <p className="px-3 pb-2 text-xs text-success">{vaultSuccess}</p>}

          {/* Dialog inline */}
          {vaultDialogMode && (
            <div className="px-3 pb-3 space-y-2 border-t border-border pt-2">
              {vaultDialogMode === 'change' ? (
                <>
                  <input
                    type="password"
                    placeholder={t("chat.vault_old_password_label")}
                    value={vaultOldPassword}
                    onChange={e => setVaultOldPassword(e.target.value)}
                    className="w-full text-sm px-2 py-1.5 rounded border border-border bg-background focus:outline-none focus:border-primary"
                  />
                  <input
                    type="password"
                    placeholder={t("chat.vault_new_password_label")}
                    value={vaultNewPassword}
                    onChange={e => setVaultNewPassword(e.target.value)}
                    className="w-full text-sm px-2 py-1.5 rounded border border-border bg-background focus:outline-none focus:border-primary"
                  />
                </>
              ) : (
                <input
                  type="password"
                  placeholder={t("chat.vault_password_label")}
                  value={vaultPassword}
                  onChange={e => setVaultPassword(e.target.value)}
                  onKeyDown={e => e.key === 'Enter' && handleVaultAction()}
                  autoFocus
                  className="w-full text-sm px-2 py-1.5 rounded border border-border bg-background focus:outline-none focus:border-primary"
                />
              )}
              <div className="flex gap-2">
                <button
                  onClick={handleVaultAction}
                  disabled={vaultLoading}
                  className="flex-1 text-sm py-1.5 rounded bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 transition-colors"
                >
                  {vaultLoading ? t("chat.vault_loading") : t("chat.vault_confirm_btn")}
                </button>
                <button
                  onClick={() => { setVaultDialogMode(null); setVaultError(null); }}
                  className="flex-1 text-sm py-1.5 rounded bg-muted hover:bg-muted/80 transition-colors"
                >
                  {t("chat.vault_cancel_btn")}
                </button>
              </div>
            </div>
          )}

          {/* Actions */}
          {!vaultDialogMode && (
            <div className="border-t border-border py-1">
              {vaultStatus?.enabled && vaultUnlocked && (
                <button
                  onClick={async () => {
                    if (!sessionToken) return;
                    await invoke("lock_vault", { sessionToken });
                    setVaultUnlocked(false);
                    setShowSelfMenu(false);
                    if (selfFriendId) loadMessages(selfFriendId);
                  }}
                  className="w-full flex items-center gap-2.5 px-3 py-2 text-sm text-foreground hover:bg-muted transition-colors text-left"
                >
                  <Lock className="w-4 h-4 text-violet-500" />
                  {t("chat.vault_lock")}
                </button>
              )}
              {!vaultStatus?.enabled && (
                <button
                  onClick={() => { setVaultDialogMode('activate'); setVaultSuccess(null); }}
                  className="w-full flex items-center gap-2.5 px-3 py-2 text-sm text-foreground hover:bg-muted transition-colors text-left"
                >
                  <Lock className="w-4 h-4 text-violet-500" />
                  {t("chat.vault_activate")}
                </button>
              )}
              {vaultStatus?.enabled && !vaultUnlocked && (
                <button
                  onClick={() => { setVaultDialogMode('unlock'); setVaultSuccess(null); }}
                  className="w-full flex items-center gap-2.5 px-3 py-2 text-sm text-foreground hover:bg-muted transition-colors text-left"
                >
                  <Lock className="w-4 h-4 text-amber-500" />
                  {t("chat.vault_unlock")}
                </button>
              )}
              {vaultStatus?.enabled && (
                <button
                  onClick={() => { setVaultDialogMode('change'); setVaultSuccess(null); }}
                  className="w-full flex items-center gap-2.5 px-3 py-2 text-sm text-foreground hover:bg-muted transition-colors text-left"
                >
                  <Timer className="w-4 h-4 text-muted-foreground" />
                  {t("chat.vault_change")}
                </button>
              )}
              {vaultStatus?.enabled && (
                <button
                  onClick={() => { setVaultDialogMode('remove'); setVaultSuccess(null); }}
                  className="w-full flex items-center gap-2.5 px-3 py-2 text-sm text-destructive hover:bg-muted transition-colors text-left"
                >
                  <X className="w-4 h-4" />
                  {t("chat.vault_remove")}
                </button>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );

  const ContactMenu = () => {
    // Pas de menu d'action pour "Mon espace"
    if (selectedFriendId === selfFriendId) return null;
    return (
      <div ref={contactMenuRef} className="relative">
        <button
          onClick={() => setShowContactMenu(v => !v)}
          className="h-8 w-8 flex items-center justify-center rounded-md text-muted-foreground hover:text-foreground hover:bg-secondary transition-colors"
        >
          <MoreVertical className="w-4 h-4" />
        </button>
        {showContactMenu && (
          <div className="absolute right-0 top-full mt-1 w-52 bg-card border border-border rounded-lg shadow-lg z-50 py-1 overflow-hidden">
            {/* Rétention DHT */}
            <div className="px-3 py-1.5">
              <p className="text-[10px] font-semibold uppercase tracking-widest text-muted-foreground mb-1.5">
                {t("chat.retention_title")}
              </p>
              <button
                onClick={() => setShowTtlMenu(v => !v)}
                className="w-full flex items-center justify-between gap-2 px-2 py-1.5 rounded-md text-sm text-foreground hover:bg-muted transition-colors"
              >
                <span className="flex items-center gap-2">
                  <Timer className="w-4 h-4 text-muted-foreground" />
                  {ttlLabel}
                </span>
                <ChevronDown className={cn("w-3 h-3 text-muted-foreground transition-transform", showTtlMenu && "rotate-180")} />
              </button>
              {showTtlMenu && (
                <div className="mt-1 border border-border rounded-md overflow-hidden">
                  {TTL_OPTIONS.map(opt => (
                    <button
                      key={opt.hours}
                      onClick={() => handleSetTtl(opt.hours)}
                      className={cn(
                        "w-full text-left px-3 py-1.5 text-sm transition-colors",
                        chatTtl === opt.hours
                          ? "bg-primary/10 text-primary font-medium"
                          : "hover:bg-muted text-foreground"
                      )}
                    >
                      {t(opt.labelKey)}
                    </button>
                  ))}
                </div>
              )}
            </div>
            <div className="my-1 border-t border-border" />
            <button
              onClick={handleBlockContact}
              className="w-full flex items-center gap-2.5 px-3 py-2 text-sm text-warning hover:bg-muted transition-colors text-left"
            >
              <Ban className="w-4 h-4" />
              Bloquer
            </button>
            <button
              onClick={handleRemoveContact}
              className="w-full flex items-center gap-2.5 px-3 py-2 text-sm text-destructive hover:bg-muted transition-colors text-left"
            >
              <Trash2 className="w-4 h-4" />
              Supprimer
            </button>
          </div>
        )}
      </div>
    );
  };

  if (!isAuthenticated) {
    return null;
  }

  return (
    <>
    <div className="h-full flex bg-background overflow-hidden max-w-full">
      {/* Modale de transfert */}
      {forwardingMessage && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
          onMouseDown={() => setForwardingMessage(null)}
        >
          <div
            className="bg-card border border-border rounded-2xl shadow-2xl w-80 max-h-[70vh] flex flex-col overflow-hidden"
            onMouseDown={e => e.stopPropagation()}
          >
            <div className="px-4 py-3 border-b border-border flex items-center justify-between">
              <span className="text-sm font-semibold text-foreground">Transférer à…</span>
              <button onClick={() => setForwardingMessage(null)} className="text-muted-foreground hover:text-foreground">
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="px-3 py-2 border-b border-border">
              <div className="relative">
                <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground" />
                <input
                  autoFocus
                  value={forwardSearch}
                  onChange={e => setForwardSearch(e.target.value)}
                  placeholder="Rechercher un contact…"
                  className="w-full pl-8 pr-3 py-1.5 text-sm bg-secondary rounded-lg border-0 focus:outline-none text-foreground placeholder:text-muted-foreground"
                />
              </div>
            </div>
            <div className="overflow-y-auto flex-1 py-1">
              {friends
                .filter(f => f.id !== selfFriendId && f.id !== selectedFriendId)
                .filter(f => f.pseudo.toLowerCase().includes(forwardSearch.toLowerCase()))
                .map(f => (
                  <button
                    key={f.id}
                    onClick={() => handleForwardTo(f.id)}
                    className="w-full flex items-center gap-3 px-4 py-2.5 hover:bg-muted transition-colors text-left"
                  >
                    <Avatar className="h-8 w-8 shrink-0">
                      {f.avatar && <AvatarImage src={`data:image/jpeg;base64,${f.avatar}`} alt={f.pseudo} />}
                      <AvatarFallback className="bg-primary text-primary-foreground text-xs font-medium">
                        {getInitials(f.pseudo)}
                      </AvatarFallback>
                    </Avatar>
                    <span className="text-sm text-foreground truncate">{f.pseudo}</span>
                  </button>
                ))}
            </div>
          </div>
        </div>
      )}

      {/* Menu long-press */}
      {longPressMenu?.visible && (
        <div
          ref={longPressMenuRef}
          className="fixed z-50 bg-card border border-border rounded-xl shadow-xl overflow-hidden"
          style={{ left: longPressMenu.x, top: longPressMenu.y, minWidth: 148 }}
        >
          <button
            onMouseDown={e => { e.stopPropagation(); handleReplyAction(); }}
            onTouchStart={e => { e.stopPropagation(); handleReplyAction(); }}
            className="flex items-center gap-3 w-full px-4 py-3 text-sm text-foreground hover:bg-muted transition-colors"
          >
            <CornerDownLeft className="h-4 w-4 text-primary" />
            Répondre
          </button>
          {!isTouchDevice && (
            <button
              onMouseDown={e => { e.stopPropagation(); handleForwardAction(); }}
              className="flex items-center gap-3 w-full px-4 py-3 text-sm text-foreground hover:bg-muted transition-colors"
            >
              <ArrowLeft className="h-4 w-4 text-muted-foreground rotate-180" />
              Transférer
            </button>
          )}
          <button
            onMouseDown={e => { e.stopPropagation(); handleDeleteAction(); }}
            onTouchStart={e => { e.stopPropagation(); handleDeleteAction(); }}
            className="flex items-center gap-3 w-full px-4 py-3 text-sm text-destructive hover:bg-muted transition-colors"
          >
            <Trash2 className="h-4 w-4" />
            Supprimer
          </button>
        </div>
      )}
      {/* Conversation List - plein écran mobile, colonne fixe desktop */}
      <div className={cn(
        "flex-shrink-0 flex-col border-r border-border bg-card",
        "w-full sm:w-72",
        selectedFriendId ? "hidden sm:flex" : "flex"
      )}>
        {/* Header */}
        <div className="p-4 border-b border-border">
          <div className="flex items-center justify-between mb-3">
            <h1 className="text-xl font-semibold text-foreground">Messages</h1>
            <div className={cn(
              "flex items-center gap-1.5 px-2 py-1 rounded-full text-xs",
              isConnected
                ? "bg-success/10 text-success"
                : "bg-muted text-muted-foreground"
            )}>
              {isConnected ? (
                <>
                  <Wifi className="w-3 h-3" />
                  <span>{t("chat.status_live")}</span>
                </>
              ) : (
                <>
                  <WifiOff className="w-3 h-3" />
                  <span>{t("chat.status_offline")}</span>
                </>
              )}
            </div>
          </div>

          {/* Search */}
          <div className="relative">
            <div className="absolute inset-y-0 left-3 flex items-center pointer-events-none text-muted-foreground">
              <Search className="w-4 h-4" />
            </div>
            <Input
              type="text"
              placeholder={t("chat.search_placeholder")}
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-10 bg-secondary border-border text-foreground placeholder:text-muted-foreground focus:border-primary"
            />
          </div>
        </div>

        {/* Conversations List */}
        <ScrollArea className="flex-1">
          {isLoading ? (
            <div className="flex items-center justify-center py-12">
              <div className="w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin" />
            </div>
          ) : filteredFriends.length === 0 ? (
            <div className="text-center py-12 px-4">
              <div className="w-12 h-12 mx-auto mb-3 rounded-full bg-secondary flex items-center justify-center text-muted-foreground">
                <MessageSquare className="w-6 h-6" />
              </div>
              <p className="text-muted-foreground text-sm">
                {searchQuery ? t("chat.no_results") : t("chat.no_conversations")}
              </p>
              <p className="text-muted-foreground text-xs mt-1">
                {t("chat.no_conversations_sub")}
              </p>
            </div>
          ) : (
            <div className="divide-y divide-border">
              {filteredFriends.map((friend, idx) => {
                const conv = conversations.get(friend.id);
                const isSelected = selectedFriendId === friend.id;
                const isSelf = friend.id === selfFriendId;
                // Séparateur entre "Mon espace" et les autres conversations
                const showDivider = idx > 0 && filteredFriends[idx - 1]?.id === selfFriendId;

                return (
                  <div key={friend.id}>
                    {showDivider && (
                      <div className="px-4 py-1.5 bg-muted/30">
                        <span className="text-[10px] font-semibold uppercase tracking-widest text-muted-foreground">
                          Conversations
                        </span>
                      </div>
                    )}
                  <button
                    key={friend.id}
                    onClick={() => {
                      setSelectedFriendId(friend.id);
                      setConversations(prev => {
                        const newMap = new Map(prev);
                        const conv = newMap.get(friend.id);
                        if (conv && conv.unreadCount > 0) {
                          newMap.set(friend.id, { ...conv, unreadCount: 0 });
                        }
                        return newMap;
                      });
                    }}
                    onContextMenu={isSelf ? undefined : (e) => {
                      e.preventDefault();
                      setFriendMenu({
                        friendId: friend.id,
                        x: e.clientX,
                        y: e.clientY,
                        isLocked: unlockedFriendIds.has(friend.id),
                      });
                    }}
                    className={cn(
                      "w-full p-4 flex items-center gap-3 transition-colors text-left",
                      isSelf && "bg-violet-500/5",
                      isSelected
                        ? "bg-primary/20 border-l-2 border-primary"
                        : conv && conv.unreadCount > 0
                          ? "border-l-2 border-white/80 hover:bg-secondary/50"
                          : "hover:bg-secondary/50"
                    )}
                  >
                    <div className="relative">
                      <Avatar className="h-12 w-12">
                        {friend.id === selfFriendId ? (
                          <AvatarFallback className="bg-gradient-to-br from-violet-600 to-indigo-500 text-white font-medium">
                            <Bookmark className="w-5 h-5" />
                          </AvatarFallback>
                        ) : (
                          <>
                            {friend.avatar && (
                              <AvatarImage src={`data:image/jpeg;base64,${friend.avatar}`} alt={friend.pseudo} />
                            )}
                            <AvatarFallback className="bg-primary text-primary-foreground font-medium">
                              {getInitials(friend.pseudo)}
                            </AvatarFallback>
                          </>
                        )}
                      </Avatar>
                      {unlockedFriendIds.has(friend.id) && (
                        <div className="absolute -bottom-0.5 -right-0.5 w-4 h-4 bg-yellow-500 rounded-full flex items-center justify-center">
                          <LockOpen className="w-2.5 h-2.5 text-white" />
                        </div>
                      )}
                      {!unlockedFriendIds.has(friend.id) && friend.verified && friend.id !== selfFriendId && (
                        <div className="absolute -bottom-0.5 -right-0.5 w-4 h-4 bg-success rounded-full flex items-center justify-center">
                          <Check className="w-3 h-3 text-success-foreground" />
                        </div>
                      )}
                    </div>

                    <div className="flex-1 min-w-0">
                      <div className="flex items-center justify-between">
                        <span className="font-medium text-foreground truncate">
                          {friend.pseudo}
                        </span>
                        <span className="text-xs text-muted-foreground">
                          {conv?.lastMessage && !(isSelf && vaultStatus?.enabled && !vaultUnlocked)
                            ? formatTime(conv.lastMessage.timestamp)
                            : !conv?.lastMessage
                              ? formatLastSeen(friend.created_at)
                              : null
                          }
                        </span>
                      </div>
                      <div className="flex items-center justify-between mt-0.5 gap-2">
                        <p className="text-sm text-muted-foreground truncate flex-1">
                          {conv?.lastMessage
                            ? (() => {
                                const msg = conv.lastMessage;
                                if (isSelf && vaultStatus?.enabled && !vaultUnlocked) return t("chat.vault_status_locked");
                                if (msg.messageType === "vault_locked") return t("chat.vault_status_locked");
                                if (msg.messageType === "audio") return t("chat.voice_message", "Vocal");
                                if (msg.messageType === "image") return t("chat.photo", "Photo");
                                if (msg.messageType === "video") return t("chat.video", "Vidéo");
                                if (msg.messageType === "pdf" || msg.messageType === "file") return msg.fileName ?? t("chat.file", "Fichier");
                                return msg.content
                                  .replace(/\[IMAGE\]:[a-f0-9]{64}/g, "🐍")
                                  .replace(/\n/g, " ")
                                  .slice(0, 30);
                              })()
                            : t("chat.no_messages")}
                        </p>
                        {conv && conv.unreadCount > 0 && (
                          <span className="shrink-0 min-w-[18px] h-[18px] flex items-center justify-center rounded-full bg-primary text-primary-foreground text-[10px] font-bold px-1">
                            {conv.unreadCount > 99 ? "99+" : conv.unreadCount}
                          </span>
                        )}
                      </div>
                    </div>
                  </button>
                  </div>
                );
              })}
            </div>
          )}
        </ScrollArea>
      </div>

      {/* Chat View - caché mobile si aucun contact sélectionné */}
      <div className={cn(
        "flex-1 min-w-0 min-h-0 flex-col bg-background",
        !selectedFriendId ? "hidden sm:flex" : "flex"
      )}>
        {selectedConversation ? (
          <>
            {/* Chat Header */}
            <div className="bg-card/95 border-b border-border">
              <div className="flex items-center justify-between px-3 sm:px-6 py-3 sm:py-4">
                <div className="flex items-center gap-2 sm:gap-4">
                  {/* Bouton retour mobile uniquement */}
                  <button
                    onClick={() => setSelectedFriendId(null)}
                    className="sm:hidden h-9 w-9 flex items-center justify-center rounded-md text-muted-foreground hover:text-foreground hover:bg-secondary transition-colors shrink-0"
                  >
                    <ArrowLeft className="w-5 h-5" />
                  </button>
                  <Avatar className="h-10 w-10 sm:h-14 sm:w-14 ring-2 ring-border shrink-0">
                    {selectedConversation.friend.id === selfFriendId ? (
                      <AvatarFallback className="bg-gradient-to-br from-violet-600 to-indigo-500 text-white font-semibold text-lg">
                        <Bookmark className="w-6 h-6" />
                      </AvatarFallback>
                    ) : (
                      <>
                        {selectedConversation.friend.avatar && (
                          <AvatarImage src={`data:image/jpeg;base64,${selectedConversation.friend.avatar}`} alt={selectedConversation.friend.pseudo} />
                        )}
                        <AvatarFallback className="bg-primary text-primary-foreground font-semibold text-lg">
                          {getInitials(selectedConversation.friend.pseudo)}
                        </AvatarFallback>
                      </>
                    )}
                  </Avatar>
                  <div className="min-w-0">
                    <div className="flex items-center gap-2 mb-0.5">
                      <span className="text-base sm:text-lg font-semibold text-foreground truncate">
                        {selectedConversation.friend.pseudo}
                      </span>
                      {selectedConversation.friend.verified && selectedConversation.friend.id !== selfFriendId && (
                        <ShieldCheck className="w-4 h-4 text-success shrink-0" />
                      )}
                    </div>
                    {selectedConversation.friend.id === selfFriendId ? (
                      <p className="text-xs text-muted-foreground">{t("chat.self_space_subtitle")}</p>
                    ) : (
                      <p className="text-xs text-muted-foreground font-mono truncate max-w-[160px] sm:max-w-xs">
                        {selectedConversation.friend.username_hash.slice(0, 24)}…
                      </p>
                    )}
                    <div className="flex items-center gap-2 mt-1">
                      <span className="flex items-center gap-1 text-success">
                        <Lock className="w-3 h-3" />
                        <span className="text-xs">{t("chat_header.e2e")}</span>
                      </span>
                      {chatTtl > 0 && selectedConversation.friend.id !== selfFriendId && (
                        <span className="flex items-center gap-1 text-amber-500">
                          <Timer className="w-3 h-3" />
                          <span className="text-xs">{ttlLabel}</span>
                        </span>
                      )}
                    </div>
                  </div>
                </div>
                <div className="flex items-center gap-1">
                  <button
                    onClick={() => setShowMessageSearch(v => !v)}
                    className={cn(
                      "h-8 w-8 flex items-center justify-center rounded-md transition-colors",
                      showMessageSearch
                        ? "text-primary bg-primary/10"
                        : "text-muted-foreground hover:text-foreground hover:bg-secondary"
                    )}
                    title="Rechercher (Ctrl+F)"
                  >
                    <Search className="w-4 h-4" />
                  </button>
                  {selectedFriendId === selfFriendId
                    ? <SelfSpaceMenu />
                    : <ContactMenu />
                  }
                </div>
              </div>
            </div>

            {/* Barre de recherche */}
            {showMessageSearch && (
              <div className="flex items-center gap-1.5 px-3 py-2 border-b border-border bg-card/80">
                <Search className="w-4 h-4 text-muted-foreground shrink-0" />
                <input
                  ref={messageSearchRef}
                  value={messageSearch}
                  onChange={e => setMessageSearch(e.target.value)}
                  onKeyDown={e => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      if (searchMatchIds.length === 0) return;
                      setCurrentMatchIdx(i => (i + 1) % searchMatchIds.length);
                    }
                  }}
                  placeholder="Rechercher dans la conversation…"
                  className="flex-1 bg-transparent text-sm text-foreground placeholder:text-muted-foreground focus:outline-none min-w-0"
                />
                {messageSearch.trim() && (
                  <>
                    <span className="text-xs text-muted-foreground shrink-0 tabular-nums">
                      {searchMatchIds.length === 0 ? "0/0" : `${currentMatchIdx + 1}/${searchMatchIds.length}`}
                    </span>
                    <button
                      onClick={() => setCurrentMatchIdx(i => Math.max(0, i - 1))}
                      disabled={currentMatchIdx === 0}
                      className="h-6 w-6 flex items-center justify-center text-muted-foreground hover:text-foreground disabled:opacity-30 transition-colors shrink-0"
                    >
                      <ChevronUp className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() => setCurrentMatchIdx(i => Math.min(searchMatchIds.length - 1, i + 1))}
                      disabled={currentMatchIdx >= searchMatchIds.length - 1}
                      className="h-6 w-6 flex items-center justify-center text-muted-foreground hover:text-foreground disabled:opacity-30 transition-colors shrink-0"
                    >
                      <ChevronDown className="w-4 h-4" />
                    </button>
                  </>
                )}
                <button
                  onClick={() => setShowMessageSearch(false)}
                  className="h-6 w-6 flex items-center justify-center text-muted-foreground hover:text-foreground transition-colors shrink-0"
                >
                  <X className="w-4 h-4" />
                </button>
              </div>
            )}

            {/* Messages Area */}
            <div className="flex-1 p-4 overflow-y-auto min-h-0 relative">
              {/* Overlay de déverrouillage vault - couvre toute la zone si Mon espace est verrouillé */}
              {selectedFriendId === selfFriendId
                && vaultStatus?.enabled
                && !vaultUnlocked
                && (
                <div className="absolute inset-0 z-10 flex flex-col items-center justify-center bg-background/95 backdrop-blur-sm">
                  <div className="w-16 h-16 rounded-full bg-gradient-to-br from-violet-600/20 to-indigo-500/20 flex items-center justify-center mb-4">
                    <Lock className="w-8 h-8 text-violet-500" />
                  </div>
                  <h3 className="text-lg font-semibold text-foreground mb-1">
                    {t("chat.vault_title")}
                  </h3>
                  <p className="text-sm text-muted-foreground mb-6 text-center max-w-xs">
                    {t("chat.vault_status_locked")}
                  </p>
                  <div className="w-64 space-y-2">
                    <input
                      type="password"
                      placeholder={t("chat.vault_password_label")}
                      value={vaultPassword}
                      onChange={e => { setVaultPassword(e.target.value); setVaultError(null); }}
                      onKeyDown={e => { if (e.key === 'Enter' && !vaultLoading && vaultPassword) doVaultUnlock(vaultPassword); }}
                      autoFocus
                      className="w-full text-sm px-3 py-2 rounded-lg border border-border bg-card focus:outline-none focus:border-violet-500"
                    />
                    {vaultError && <p className="text-xs text-destructive text-center">{vaultError}</p>}
                    <button
                      disabled={vaultLoading || !vaultPassword}
                      onClick={() => doVaultUnlock(vaultPassword)}
                      className="w-full py-2 rounded-lg bg-violet-600 text-white text-sm font-medium hover:bg-violet-700 disabled:opacity-50 transition-colors"
                    >
                      {vaultLoading ? t("chat.vault_loading") : t("chat.vault_unlock")}
                    </button>
                  </div>
                </div>
              )}

              {selectedConversation.messages.length === 0 ? (
                <div className="h-full flex flex-col items-center justify-center text-center px-4">
                  <div className="w-20 h-20 rounded-full bg-secondary flex items-center justify-center mb-4">
                    <MessageSquare className="w-10 h-10 text-primary" />
                  </div>
                  <h3 className="text-lg font-medium text-foreground mb-1">
                    {t("chat.start_conversation")}
                  </h3>
                  <p className="text-sm text-muted-foreground max-w-xs">
                    {t("chat.send_message_to", { name: selectedConversation.friend.pseudo })}
                    {" "}{t("chat.e2e_notice")}
                  </p>
                  <div className="flex items-center gap-2 mt-4 text-xs text-muted-foreground">
                    <Lock className="w-3 h-3" />
                    <span>{t("chat.pq_active")}</span>
                  </div>
                </div>
              ) : (
                <div className="space-y-3">
                  {selectedConversation.messages.map((message) => {
                    const isUser = message.sender === "user";
                    const isCurrentMatch = searchMatchIds[currentMatchIdx] === message.id;

                    return (
                      <div
                        key={message.id}
                        id={`msg-${message.id}`}
                        className={cn(
                          "flex",
                          isUser ? "justify-end" : "justify-start",
                          isCurrentMatch && messageSearch.trim() && "rounded-xl ring-2 ring-primary/50"
                        )}
                      >
                        {/* Dialog de confirmation suppression */}
                        {deletingMessageId === message.id && (
                          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
                            onClick={() => setDeletingMessageId(null)}>
                            <div className="bg-card border border-border rounded-2xl p-5 mx-6 shadow-2xl"
                              onClick={e => e.stopPropagation()}>
                              <p className="text-sm font-semibold text-foreground mb-1">Supprimer ce message ?</p>
                              <p className="text-xs text-muted-foreground mb-4">Cette action est définitive. Le contenu sera écrasé de façon sécurisée.</p>
                              <div className="flex gap-2 justify-end">
                                <button onClick={() => setDeletingMessageId(null)}
                                  className="px-3 py-1.5 text-xs rounded-lg bg-secondary text-foreground">
                                  Annuler
                                </button>
                                <button onClick={() => handleDeleteMessage(message.id)}
                                  className="px-3 py-1.5 text-xs rounded-lg bg-destructive text-white">
                                  Supprimer définitivement
                                </button>
                              </div>
                            </div>
                          </div>
                        )}
                        <div
                          className={cn(
                            `max-w-[75%] px-4 py-2.5 ${bubbleRadius} break-words overflow-hidden`,
                            isUser
                              ? "bg-primary text-primary-foreground"
                              : "bg-secondary text-foreground"
                          )}
                          onContextMenu={e => { e.preventDefault(); startLongPress(message.id, e.clientX, e.clientY); cancelLongPress(); const x = Math.min(e.clientX, window.innerWidth - 160); const y = Math.max(e.clientY - 90, 8); setLongPressMenu({ visible: true, x, y, messageId: message.id }); }}
                          onMouseDown={e => { if (e.button !== 0) return; startLongPress(message.id, e.clientX, e.clientY); }}
                          onMouseUp={cancelLongPress}
                          onMouseLeave={cancelLongPress}
                          onTouchStart={e => { const t = e.touches[0]; startLongPress(message.id, t.clientX, t.clientY); }}
                          onTouchEnd={cancelLongPress}
                          onTouchMove={cancelLongPress}
                          style={{ WebkitUserSelect: "none", userSelect: "none" }}
                        >
                          {/* Citation de réponse */}
                          {message.replyToId && (() => {
                            const src = selectedConversation?.messages.find(m => m.id === message.replyToId);
                            return (
                              <div className={cn(
                                "text-xs px-2 py-1 rounded-md mb-1.5 border-l-2 opacity-80 max-w-full",
                                isUser
                                  ? "bg-white/10 border-white/50 text-white/80"
                                  : "bg-black/10 border-primary/50 text-foreground/70"
                              )}>
                                <p className="font-medium text-[10px] mb-0.5 opacity-70">
                                  {src?.sender === "user" ? "Vous" : selectedConversation?.friend.pseudo ?? "?"}
                                </p>
                                <p className="truncate text-[11px]">
                                  {src?.content?.slice(0, 60) ?? "Message indisponible"}
                                  {(src?.content?.length ?? 0) > 60 ? "…" : ""}
                                </p>
                              </div>
                            );
                          })()}

                          <div className="text-sm leading-relaxed whitespace-pre-wrap">
                            {message.messageType === "image" && message.fileData ? (
                              <img
                                src={`data:${message.fileMime ?? "image/png"};base64,${message.fileData}`}
                                alt={message.fileName ?? "image"}
                                className="max-w-full rounded-lg max-h-64 object-contain cursor-zoom-in"
                                onClick={() => setLightbox({
                                  src: `data:${message.fileMime ?? "image/png"};base64,${message.fileData}`,
                                  alt: message.fileName ?? "image",
                                })}
                              />
                            ) : message.messageType === "audio" && message.fileData ? (
                              <AudioPlayer
                                src={message.fileData}
                                mime={message.fileMime ?? "audio/webm"}
                                isOwn={message.sender === "user"}
                              />
                            ) : message.messageType === "video" && message.fileData ? (
                              <video
                                controls
                                src={`data:${message.fileMime ?? "video/mp4"};base64,${message.fileData}`}
                                className="max-w-full rounded-lg max-h-64"
                              />
                            ) : (message.messageType === "pdf" || message.messageType === "file") && message.fileData ? (
                              <button
                                onClick={() => {
                                    const raw = atob(message.fileData!);
                                    const bytes = new Uint8Array(raw.length);
                                    for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
                                    const blob = new Blob([bytes], { type: message.fileMime ?? "application/octet-stream" });
                                    const url = URL.createObjectURL(blob);
                                    const a = document.createElement("a");
                                    a.href = url;
                                    a.download = message.fileName ?? "file";
                                    document.body.appendChild(a);
                                    a.click();
                                    document.body.removeChild(a);
                                    URL.revokeObjectURL(url);
                                }}
                                className="flex items-center gap-2 underline underline-offset-2 opacity-90 hover:opacity-100 cursor-pointer bg-transparent border-0 p-0 text-inherit"
                              >
                                <FileIcon className="w-4 h-4 shrink-0" />
                                <span>{message.fileName ?? "Télécharger"}</span>
                              </button>
                            ) : (
                              parseMessageContent(message.content).map((seg, i) =>
                                seg.type === "text" ? (
                                  <span key={i}>{highlightText(seg.content, isCurrentMatch)}</span>
                                ) : (
                                  <img
                                    key={i}
                                    src={seg.url}
                                    alt={seg.hash}
                                    className="inline-block h-20 w-20 object-contain align-middle mx-0.5"
                                  />
                                )
                              )
                            )}
                          </div>
                          <div className={cn(
                            "flex items-center gap-1.5 mt-1",
                            isUser ? "justify-end" : "justify-start"
                          )}>
                            <span className={cn(
                              "text-[10px]",
                              isUser ? "text-primary-foreground/70" : "text-muted-foreground"
                            )}>
                              {formatTime(message.timestamp)}
                            </span>
                            {isUser && message.status && (
                              <span className="text-primary-foreground/70">
                                {message.status === "sending" ? (
                                  <div className="w-3 h-3 border border-current border-t-transparent rounded-full animate-spin" />
                                ) : message.status === "read" ? (
                                  <CheckCheck className="w-3 h-3" />
                                ) : (
                                  <Check className="w-3 h-3" />
                                )}
                              </span>
                            )}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                  <div ref={messagesEndRef} />
                </div>
              )}
            </div>

            {/* Input Area */}
            <div className="p-3 bg-card/95 border-t border-border">

              {/* Barre de réponse */}
              {replyingTo && (
                <div className="flex items-center gap-2 mb-2 px-2 py-1.5 bg-secondary/60 rounded-lg border-l-2 border-primary">
                  <CornerDownLeft className="w-3.5 h-3.5 text-primary shrink-0" />
                  <div className="flex-1 min-w-0">
                    <p className="text-xs font-medium text-primary leading-tight">
                      {replyingTo.sender === "user" ? "Vous" : selectedConversation?.friend.pseudo}
                    </p>
                    <p className="text-xs text-muted-foreground truncate">
                      {replyingTo.content.slice(0, 60)}{replyingTo.content.length > 60 ? "…" : ""}
                    </p>
                  </div>
                  <button
                    onClick={() => setReplyingTo(null)}
                    className="text-muted-foreground hover:text-foreground transition-colors shrink-0"
                  >
                    <X className="w-3.5 h-3.5" />
                  </button>
                </div>
              )}

              {/* Badge fichier en attente */}
              {(pendingFile || isSanitizing) && (
                <div className="flex items-center gap-2 mb-2 px-1">
                  {isSanitizing ? (
                    <div className="flex items-center gap-2 text-xs text-muted-foreground">
                      <Loader2 className="w-3.5 h-3.5 animate-spin" />
                      {t("chat.sanitizing")}
                    </div>
                  ) : pendingFile && (
                    <div className="flex items-center gap-2 w-full">
                      {pendingAudioUrl ? (
                        /* Preview vocal avant envoi */
                        <div className="flex items-center gap-2 flex-1">
                          <AudioPlayer src={pendingAudioUrl} mime={pendingFile.mimeType} />
                          <button
                            onClick={() => { setPendingFile(null); URL.revokeObjectURL(pendingAudioUrl); setPendingAudioUrl(null); }}
                            className="text-muted-foreground hover:text-destructive transition-colors shrink-0"
                          >
                            <X className="w-4 h-4" />
                          </button>
                        </div>
                      ) : (
                        <div className="flex items-center gap-2 bg-secondary rounded-xl px-3 py-1.5 text-xs max-w-xs">
                          <FileIcon className="w-3.5 h-3.5 text-accent-secondary shrink-0" />
                          <span className="truncate text-foreground">{pendingFile.name}</span>
                          <span className="text-muted-foreground shrink-0">
                            {(pendingFile.sanitizedSize / 1024).toFixed(1)} Ko
                          </span>
                          <button
                            onClick={() => setPendingFile(null)}
                            className="text-muted-foreground hover:text-destructive transition-colors shrink-0"
                          >
                            <X className="w-3 h-3" />
                          </button>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              )}

              {/* Caméra inline - évite le sélecteur Android (GrapheneOS, etc.) */}
              {showCamera && (
                <div className="fixed inset-0 z-50 bg-black flex flex-col">
                  <div className="flex items-center justify-between px-4 py-3">
                    <button onClick={closeCamera} className="text-white">
                      <X className="w-6 h-6" />
                    </button>
                    <span className="text-white text-sm font-medium">Photo</span>
                    <div className="w-6" />
                  </div>
                  <video
                    ref={cameraVideoRef}
                    className="flex-1 w-full object-cover"
                    muted
                    playsInline
                  />
                  <div className="flex justify-center items-center py-8">
                    <button
                      onClick={capturePhoto}
                      className="w-16 h-16 rounded-full bg-white border-4 border-white/50 active:scale-95 transition-transform"
                    />
                  </div>
                </div>
              )}

              {/* Picker emotes mobile - fixed pour éviter les problèmes de positionnement */}
              {isTouchDevice && showEmotePicker && (
                <div
                  className="fixed inset-x-0 bottom-0 z-50 bg-card border-t border-border shadow-2xl rounded-t-2xl"
                  style={{ paddingBottom: "env(safe-area-inset-bottom)" }}
                >
                  <div className="flex items-center justify-between px-4 py-2 border-b border-border">
                    <span className="text-xs font-medium text-foreground">Emotes</span>
                    <button onClick={() => setShowEmotePicker(false)} className="text-muted-foreground hover:text-foreground">
                      <X className="w-4 h-4" />
                    </button>
                  </div>
                  <div className="p-3 overflow-y-auto" style={{ maxHeight: 260 }}>
                    <div className="grid grid-cols-5 gap-2">
                      {Object.entries(EMOTE_MAP).map(([hash, filename]) => (
                        <button
                          key={hash}
                          onClick={() => { insertEmote(hash); setShowEmotePicker(false); }}
                          className="p-1.5 rounded-lg hover:bg-secondary transition-colors flex items-center justify-center"
                        >
                          <img src={`/emotes/${filename}`} alt={filename} className="w-12 h-12 object-contain" />
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
              )}

              {/* Inputs cachés */}
              <input
                ref={fileInputRef}
                type="file"
                className="hidden"
                onChange={handleFileSelect}
                accept="image/*,video/*,audio/*,application/pdf,.pdf"
              />
              <input
                ref={cameraInputRef}
                type="file"
                className="hidden"
                onChange={handleFileSelect}
                accept="image/*"
                capture="environment"
              />

              <div className="flex items-end gap-2 bg-secondary rounded-2xl px-3 py-2">
                {/* Mobile : grille 2×2 [📷][📎] / [😊][🎤] */}
                {isTouchDevice ? (
                  <div className="grid grid-cols-2 gap-0.5 shrink-0">
                    {/* Caméra */}
                    <Button variant="ghost" size="icon"
                      onClick={openCamera}
                      disabled={isSanitizing || isRecording}
                      className="h-8 w-8 text-muted-foreground hover:text-foreground hover:bg-muted"
                    >
                      <Camera className="w-4 h-4" />
                    </Button>
                    {/* Fichier */}
                    <Button variant="ghost" size="icon"
                      onClick={() => fileInputRef.current?.click()}
                      disabled={isSanitizing || isRecording}
                      className={cn("h-8 w-8 hover:bg-muted transition-colors",
                        pendingFile ? "text-accent-secondary" : "text-muted-foreground hover:text-foreground"
                      )}
                    >
                      {isSanitizing ? <Loader2 className="w-4 h-4 animate-spin" /> : <Paperclip className="w-4 h-4" />}
                    </Button>
                    {/* Emotes */}
                    <Button variant="ghost" size="icon"
                      onClick={() => setShowEmotePicker(v => !v)}
                      disabled={isRecording}
                      className={cn("h-8 w-8 hover:bg-muted transition-colors",
                        showEmotePicker ? "text-primary" : "text-muted-foreground hover:text-foreground"
                      )}
                    >
                      <Smile className="w-4 h-4" />
                    </Button>
                    {/* Micro */}
                    {isRecording ? (
                      <Button size="icon"
                        onClick={async () => {
                          const file = await stopRecording();
                          if (file) {
                            // Preview blob URL avant envoi
                            const url = URL.createObjectURL(file);
                            if (pendingAudioUrl) URL.revokeObjectURL(pendingAudioUrl);
                            setPendingAudioUrl(url);
                            await processFile(file);
                          }
                        }}
                        className="h-8 w-8 bg-primary hover:bg-primary/90 text-white rounded-full"
                      >
                        <Square className="w-3 h-3 fill-current" />
                      </Button>
                    ) : (
                      <Button variant="ghost" size="icon"
                        onClick={async () => {
                          const ok = await startRecording();
                          if (!ok) alert("Microphone inaccessible. Vérifiez les permissions.");
                        }}
                        disabled={isSanitizing}
                        className="h-8 w-8 text-muted-foreground hover:text-foreground hover:bg-muted"
                      >
                        <Mic className="w-4 h-4" />
                      </Button>
                    )}
                  </div>
                ) : (
                  /* Desktop : bouton fichier horizontal */
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => fileInputRef.current?.click()}
                    disabled={isSanitizing}
                    className={cn(
                      "h-9 w-9 shrink-0 hover:bg-secondary transition-colors",
                      pendingFile ? "text-accent-secondary" : "text-muted-foreground hover:text-foreground"
                    )}
                  >
                    {isSanitizing
                      ? <Loader2 className="w-5 h-5 animate-spin" />
                      : <Paperclip className="w-5 h-5" />
                    }
                  </Button>
                )}

                {/* Emote/Emoji button + picker - desktop uniquement */}
                {!isTouchDevice && <div ref={emoteWrapperRef} className="relative shrink-0">
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => setShowEmotePicker(v => !v)}
                    className={cn(
                      "h-9 w-9 hover:bg-secondary transition-colors",
                      showEmotePicker ? "text-primary" : "text-muted-foreground hover:text-foreground"
                    )}
                  >
                    <Smile className="w-5 h-5" />
                  </Button>
                  {showEmotePicker && (
                    <div className="absolute bottom-full left-0 mb-2 bg-card border border-border rounded-2xl shadow-lg z-50 overflow-hidden">
                      {/* Onglets : Emojis masqué sur mobile */}
                      <div className="flex border-b border-border">
                        {!isTouchDevice && (
                          <button
                            onClick={() => setEmojiTab("emoji")}
                            className={cn(
                              "flex-1 py-2 text-xs font-medium transition-colors",
                              emojiTab === "emoji"
                                ? "text-primary border-b-2 border-primary"
                                : "text-muted-foreground hover:text-foreground"
                            )}
                          >
                            😊 Emojis
                          </button>
                        )}
                        <button
                          onClick={() => setEmojiTab("emote")}
                          className={cn(
                            "flex-1 py-2 text-xs font-medium transition-colors",
                            emojiTab === "emote"
                              ? "text-primary border-b-2 border-primary"
                              : "text-muted-foreground hover:text-foreground"
                          )}
                        >
                          🐍 Emotes
                        </button>
                      </div>

                      {emojiTab === "emoji" && !isTouchDevice ? (
                        <Suspense fallback={<div style={{ width: 320, height: 380 }} />}>
                          <LazyEmojiPicker
                            onEmojiClick={insertEmoji}
                            theme={"dark" as any}
                            width={320}
                            height={380}
                            searchPlaceholder="Rechercher…"
                            previewConfig={{ showPreview: false }}
                          />
                        </Suspense>
                      ) : (
                        <div className="p-3" style={{ width: 320, height: 380, overflowY: "auto" }}>
                          <div className="grid grid-cols-5 gap-2">
                            {Object.entries(EMOTE_MAP).map(([hash, filename]) => (
                              <button
                                key={hash}
                                onClick={() => insertEmote(hash)}
                                className="p-1.5 rounded-lg hover:bg-secondary transition-colors flex items-center justify-center"
                                title={filename.replace("snake_", "").replace(".png", "")}
                              >
                                <img
                                  src={`/emotes/${filename}`}
                                  alt={filename}
                                  className="w-12 h-12 object-contain"
                                />
                              </button>
                            ))}
                          </div>
                        </div>
                      )}
                    </div>
                  )}
                </div>}


                {isRecording ? (
                  <VoiceWaveform
                    analyserRef={analyserRef}
                    duration={duration}
                    formatDuration={formatDuration}
                  />
                ) : (
                  <textarea
                    ref={textareaRef}
                    value={messageInput}
                    onChange={(e) => setMessageInput(e.target.value)}
                    onKeyDown={handleKeyDown}
                    disabled={selectedFriendId === selfFriendId && vaultStatus?.enabled && !vaultUnlocked}
                    placeholder={
                      selectedFriendId === selfFriendId && vaultStatus?.enabled && !vaultUnlocked
                        ? t("chat.vault_status_locked")
                        : t("chat.input_placeholder")
                    }
                    rows={1}
                    className="flex-1 bg-transparent text-foreground placeholder-muted-foreground resize-none focus:outline-none text-sm leading-relaxed py-1.5 disabled:opacity-40 disabled:cursor-not-allowed"
                  />
                )}

                {/* Toggle Entrée envoie / retour à la ligne */}
                <button
                  onClick={() => setEnterToSend(v => !v)}
                  title={enterToSend ? "Entrée = Envoyer (cliquer pour changer)" : "Entrée = Nouvelle ligne (cliquer pour changer)"}
                  className={cn(
                    "h-9 w-9 shrink-0 flex items-center justify-center rounded-full transition-colors",
                    enterToSend ? "text-primary" : "text-muted-foreground hover:text-foreground"
                  )}
                >
                  <CornerDownLeft className="w-4 h-4" />
                </button>

                <Button
                  onClick={handleSendMessage}
                  disabled={(!messageInput.trim() && !pendingFile) || (selectedFriendId === selfFriendId && vaultStatus?.enabled && !vaultUnlocked)}
                  size="icon"
                  className={cn(
                    "h-9 w-9 rounded-full transition-all duration-200",
                    (messageInput.trim() || pendingFile)
                      ? "bg-primary hover:bg-primary/90 text-primary-foreground hover:scale-105"
                      : "bg-muted text-muted-foreground cursor-not-allowed"
                  )}
                >
                  <Send className="w-4 h-4" />
                </Button>
              </div>

              <div className="flex items-center justify-center gap-1.5 mt-2 text-muted-foreground">
                <Lock className="w-3 h-3" />
                <span className="text-[10px]">{t("chat.e2e_notice")}</span>
              </div>
            </div>
          </>
        ) : (
          <div className="h-full flex flex-col items-center justify-center text-center px-4">
            <div className="w-24 h-24 rounded-full bg-secondary flex items-center justify-center mb-6">
              <MessageSquare className="w-12 h-12 text-primary/50" />
            </div>
            <h2 className="text-xl font-semibold text-foreground mb-2">
              {t("chat.title")}
            </h2>
            <p className="text-muted-foreground text-sm max-w-sm">
              {t("chat.select_conversation")} {t("chat.select_conversation_sub")}
            </p>
            <div className="flex items-center gap-2 mt-6 px-4 py-2 rounded-full bg-secondary/50 border border-border">
              <span className="text-success">
                <ShieldCheck className="w-4 h-4" />
              </span>
              <span className="text-xs text-muted-foreground">{t("chat.pq_label")}</span>
            </div>
          </div>
        )}
      </div>
    </div>

    {/* Menu contextuel sur une conv (verrouiller / déverrouiller) */}
    {friendMenu && (
      <div
        ref={friendMenuRef}
        style={{ position: "fixed", top: friendMenu.y, left: friendMenu.x, zIndex: 9999 }}
        className="bg-card border border-border rounded-lg shadow-xl py-1 min-w-[160px] text-sm"
      >
        {friendMenu.isLocked ? (
          <button
            className="w-full flex items-center gap-2 px-4 py-2 hover:bg-secondary transition-colors text-left text-foreground"
            onClick={() => {
              setRemoveLockModal({ friendId: friendMenu.friendId, pin: "", error: null, loading: false });
              setFriendMenu(null);
            }}
          >
            <LockOpen className="w-4 h-4 text-yellow-500" />
            {t("chat.remove_lock")}
          </button>
        ) : (
          <button
            className="w-full flex items-center gap-2 px-4 py-2 hover:bg-secondary transition-colors text-left text-foreground"
            onClick={() => {
              setLockModal({ friendId: friendMenu.friendId, step: 'enter', pin: "", confirm: "", error: null, loading: false });
              setFriendMenu(null);
            }}
          >
            <LockKeyhole className="w-4 h-4 text-primary" />
            {t("chat.lock_conversation")}
          </button>
        )}
      </div>
    )}

    {/* Modal : définir un PIN */}
    {lockModal && (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
        <div className="bg-card border border-border rounded-xl shadow-2xl p-6 w-80 space-y-4">
          <div className="flex items-center gap-2">
            <LockKeyhole className="w-5 h-5 text-primary" />
            <h2 className="text-foreground font-semibold">{t("chat.lock_modal_title")}</h2>
          </div>
          {lockModal.step === 'enter' ? (
            <>
              <p className="text-sm text-muted-foreground">{t("chat.lock_modal_desc")}</p>
              <input
                autoFocus
                type="password"
                placeholder={t("chat.lock_modal_pin_placeholder")}
                className="w-full bg-secondary border border-border rounded-md px-3 py-2 text-foreground text-sm focus:outline-none focus:border-primary"
                value={lockModal.pin}
                onChange={e => setLockModal(prev => prev ? { ...prev, pin: e.target.value, error: null } : null)}
                onKeyDown={e => {
                  if (e.key === "Enter" && lockModal.pin.length >= 4) {
                    setLockModal(prev => prev ? { ...prev, step: 'confirm' } : null);
                  }
                }}
              />
              {lockModal.error && <p className="text-xs text-destructive">{lockModal.error}</p>}
              <div className="flex gap-2 justify-end">
                <button
                  className="px-4 py-2 text-sm rounded-md border border-border hover:bg-secondary text-foreground"
                  onClick={() => setLockModal(null)}
                >{t("common.cancel")}</button>
                <button
                  className="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
                  disabled={lockModal.pin.length < 4}
                  onClick={() => setLockModal(prev => prev ? { ...prev, step: 'confirm' } : null)}
                >{t("chat.lock_modal_next")}</button>
              </div>
            </>
          ) : (
            <>
              <p className="text-sm text-muted-foreground">{t("chat.lock_modal_confirm_placeholder")}</p>
              <input
                autoFocus
                type="password"
                placeholder={t("chat.lock_modal_confirm_placeholder")}
                className="w-full bg-secondary border border-border rounded-md px-3 py-2 text-foreground text-sm focus:outline-none focus:border-primary"
                value={lockModal.confirm}
                onChange={e => setLockModal(prev => prev ? { ...prev, confirm: e.target.value, error: null } : null)}
                onKeyDown={async e => {
                  if (e.key === "Enter") {
                    if (lockModal.confirm !== lockModal.pin) {
                      setLockModal(prev => prev ? { ...prev, error: t("chat.lock_modal_pin_mismatch") } : null);
                      return;
                    }
                    if (!sessionToken) return;
                    setLockModal(prev => prev ? { ...prev, loading: true } : null);
                    try {
                      await invoke('lock_conversation', { sessionToken, friendId: lockModal.friendId, pin: lockModal.pin });
                      // Retire l'ami de la liste
                      setFriends(prev => prev.filter(f => f.id !== lockModal.friendId));
                      setUnlockedLockedFriends(prev => prev.filter(f => f.id !== lockModal.friendId));
                      setUnlockedFriendIds(prev => { const s = new Set(prev); s.delete(lockModal.friendId); return s; });
                      if (selectedFriendId === lockModal.friendId) setSelectedFriendId(null);
                      setLockModal(null);
                    } catch (err) {
                      setLockModal(prev => prev ? { ...prev, error: String(err), loading: false } : null);
                    }
                  }
                }}
              />
              {lockModal.error && <p className="text-xs text-destructive">{lockModal.error}</p>}
              <div className="flex gap-2 justify-end">
                <button
                  className="px-4 py-2 text-sm rounded-md border border-border hover:bg-secondary text-foreground"
                  onClick={() => setLockModal(prev => prev ? { ...prev, step: 'enter', confirm: "" } : null)}
                >{t("chat.lock_modal_back")}</button>
                <button
                  disabled={lockModal.loading || lockModal.confirm.length < 4}
                  className="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 flex items-center gap-2"
                  onClick={async () => {
                    if (lockModal.confirm !== lockModal.pin) {
                      setLockModal(prev => prev ? { ...prev, error: t("chat.lock_modal_pin_mismatch") } : null);
                      return;
                    }
                    if (!sessionToken) return;
                    setLockModal(prev => prev ? { ...prev, loading: true } : null);
                    try {
                      await invoke('lock_conversation', { sessionToken, friendId: lockModal.friendId, pin: lockModal.pin });
                      setFriends(prev => prev.filter(f => f.id !== lockModal.friendId));
                      setUnlockedLockedFriends(prev => prev.filter(f => f.id !== lockModal.friendId));
                      setUnlockedFriendIds(prev => { const s = new Set(prev); s.delete(lockModal.friendId); return s; });
                      if (selectedFriendId === lockModal.friendId) setSelectedFriendId(null);
                      setLockModal(null);
                    } catch (err) {
                      setLockModal(prev => prev ? { ...prev, error: String(err), loading: false } : null);
                    }
                  }}
                >
                  {lockModal.loading && <Loader2 className="w-4 h-4 animate-spin" />}
                  {t("chat.lock_modal_confirm")}
                </button>
              </div>
            </>
          )}
        </div>
      </div>
    )}

    {/* Modal : retirer le verrou */}
    {removeLockModal && (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
        <div className="bg-card border border-border rounded-xl shadow-2xl p-6 w-80 space-y-4">
          <div className="flex items-center gap-2">
            <LockOpen className="w-5 h-5 text-yellow-500" />
            <h2 className="text-foreground font-semibold">{t("chat.remove_lock_title")}</h2>
          </div>
          <p className="text-sm text-muted-foreground">{t("chat.remove_lock_desc")}</p>
          <input
            autoFocus
            type="password"
            placeholder={t("chat.remove_lock_pin_placeholder")}
            className="w-full bg-secondary border border-border rounded-md px-3 py-2 text-foreground text-sm focus:outline-none focus:border-primary"
            value={removeLockModal.pin}
            onChange={e => setRemoveLockModal(prev => prev ? { ...prev, pin: e.target.value, error: null } : null)}
            onKeyDown={async e => { if (e.key === "Enter") await handleRemoveLock(); }}
          />
          {removeLockModal.error && <p className="text-xs text-destructive">{removeLockModal.error}</p>}
          <div className="flex gap-2 justify-end">
            <button
              className="px-4 py-2 text-sm rounded-md border border-border hover:bg-secondary text-foreground"
              onClick={() => setRemoveLockModal(null)}
            >{t("common.cancel")}</button>
            <button
              disabled={removeLockModal.loading || removeLockModal.pin.length < 4}
              className="px-4 py-2 text-sm rounded-md bg-yellow-500 text-white hover:bg-yellow-600 disabled:opacity-50 flex items-center gap-2"
              onClick={handleRemoveLock}
            >
              {removeLockModal.loading && <Loader2 className="w-4 h-4 animate-spin" />}
              {t("chat.remove_lock_confirm")}
            </button>
          </div>
        </div>
      </div>
    )}

    {fileSendBanner && (
      <div className="fixed top-4 right-4 z-50 animate-in slide-in-from-top-2 fade-in duration-300 max-w-xs">
        <div className={`px-4 py-3 rounded-lg shadow-lg flex items-center gap-2 text-sm ${
          fileSendBanner.done
            ? "bg-green-600 text-white"
            : "bg-yellow-500 text-white"
        }`}>
          {fileSendBanner.done
            ? <Check className="w-4 h-4 shrink-0" />
            : <Loader2 className="w-4 h-4 shrink-0 animate-spin" />
          }
          <span>{fileSendBanner.message}</span>
        </div>
      </div>
    )}

    {lightbox && (
      <ImageLightbox
        src={lightbox.src}
        alt={lightbox.alt}
        onClose={() => setLightbox(null)}
      />
    )}
    </>
  );
}
