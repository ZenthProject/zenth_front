import { useRef, useState, useEffect, useCallback } from "react";
import { Check, CheckCheck, Clock, AlertCircle, Reply, Trash2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { useTranslation } from "react-i18next";

const IS_MOBILE = /android|iphone|ipad/i.test(navigator.userAgent);

export interface Message {
  id: string;
  content: string;
  sender: "user" | "assistant" | "system";
  timestamp: Date;
  avatar?: string;
  senderName?: string;
  status?: "sending" | "sent" | "delivered" | "read" | "error";
  replyTo?: {
    id: string;
    content: string;
    senderName: string;
  };
  isEdited?: boolean;
  messageType?: string;
  fileName?: string | null;
  fileMime?: string | null;
  fileData?: string | null;
}

interface MessageBubbleProps {
  message: Message;
  showAvatar?: boolean;
  showTimestamp?: boolean;
  isFirstInGroup?: boolean;
  isLastInGroup?: boolean;
  onAction?: (messageId: string, action: string) => void;
  onReply?: (message: Message) => void;
}

interface MenuState {
  visible: boolean;
  x: number;
  y: number;
}

export function MessageBubble({
  message,
  showAvatar = true,
  showTimestamp = true,
  isFirstInGroup = true,
  isLastInGroup = true,
  onAction,
  onReply,
}: MessageBubbleProps) {
  const { t } = useTranslation();
  const isUser = message.sender === "user";
  const isSystem = message.sender === "system";
  const hasActions = !!(onAction || onReply);

  const [menu, setMenu] = useState<MenuState>({ visible: false, x: 0, y: 0 });
  const longPressTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const isEmojiOnly = IS_MOBILE
    ? false
    : /^[\u{1F000}-\u{1FFFF}\u{2600}-\u{27BF}\u{2300}-\u{23FF}\u{200D}\u{FE0F}\u{20E3}\s]+$/u.test(
        message.content.trim()
      ) && message.content.trim().length > 0;

  const formatTime = (date: Date) =>
    date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

  const getInitials = (name: string) =>
    name?.split(" ").map((n) => n[0]).join("").toUpperCase().slice(0, 2) || "?";

  // Ferme le menu au clic extérieur
  useEffect(() => {
    if (!menu.visible) return;
    const handler = (e: MouseEvent | TouchEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenu((m) => ({ ...m, visible: false }));
      }
    };
    document.addEventListener("mousedown", handler);
    document.addEventListener("touchstart", handler);
    return () => {
      document.removeEventListener("mousedown", handler);
      document.removeEventListener("touchstart", handler);
    };
  }, [menu.visible]);

  const openMenu = useCallback((clientX: number, clientY: number) => {
    if (!hasActions) return;
    // Ajuste pour ne pas sortir de l'écran
    const x = Math.min(clientX, window.innerWidth - 160);
    const y = Math.max(clientY - 90, 8);
    setMenu({ visible: true, x, y });
  }, [hasActions]);

  const startLongPress = (clientX: number, clientY: number) => {
    if (!hasActions) return;
    longPressTimer.current = setTimeout(() => openMenu(clientX, clientY), 450);
  };

  const cancelLongPress = () => {
    if (longPressTimer.current) {
      clearTimeout(longPressTimer.current);
      longPressTimer.current = null;
    }
  };

  const handleMouseDown = (e: React.MouseEvent) => startLongPress(e.clientX, e.clientY);
  const handleTouchStart = (e: React.TouchEvent) => {
    const t = e.touches[0];
    startLongPress(t.clientX, t.clientY);
  };

  const handleReply = () => {
    setMenu((m) => ({ ...m, visible: false }));
    onReply?.(message);
  };

  const handleDelete = () => {
    setMenu((m) => ({ ...m, visible: false }));
    onAction?.(message.id, "delete");
  };

  const StatusIcon = () => {
    switch (message.status) {
      case "sending":   return <Clock      className="h-3 w-3 text-primary-foreground/50" />;
      case "sent":      return <Check      className="h-3 w-3 text-primary-foreground/50" />;
      case "delivered": return <CheckCheck className="h-3 w-3 text-primary-foreground/60" />;
      case "read":      return <CheckCheck className="h-3 w-3 text-primary-foreground" />;
      case "error":     return <AlertCircle className="h-3 w-3 text-red-500" />;
      default:          return null;
    }
  };

  if (isSystem) {
    return (
      <div className="flex justify-center my-4">
        <div className="bg-muted/50 text-muted-foreground text-xs px-4 py-2 rounded-full border border-border/50">
          {message.content}
        </div>
      </div>
    );
  }

  return (
    <>
      {/* Menu long-press */}
      {menu.visible && (
        <div
          ref={menuRef}
          className="fixed z-50 bg-popover border border-border rounded-xl shadow-xl overflow-hidden"
          style={{ left: menu.x, top: menu.y, minWidth: 148 }}
        >
          {onReply && (
            <button
              onMouseDown={(e) => { e.stopPropagation(); handleReply(); }}
              onTouchStart={(e) => { e.stopPropagation(); handleReply(); }}
              className="flex items-center gap-3 w-full px-4 py-3 text-sm text-foreground hover:bg-muted active:bg-muted/80 transition-colors"
            >
              <Reply className="h-4 w-4 text-primary" />
              {t("chat.reply")}
            </button>
          )}
          {onAction && (
            <button
              onMouseDown={(e) => { e.stopPropagation(); handleDelete(); }}
              onTouchStart={(e) => { e.stopPropagation(); handleDelete(); }}
              className="flex items-center gap-3 w-full px-4 py-3 text-sm text-red-400 hover:bg-red-900/20 active:bg-red-900/40 transition-colors"
            >
              <Trash2 className="h-4 w-4" />
              {t("chat.delete")}
            </button>
          )}
        </div>
      )}

      {/* Bulle */}
      <div
        className={cn("flex", isUser ? "justify-end" : "justify-start")}
        onContextMenu={e => { e.preventDefault(); if (hasActions) { const x = Math.min(e.clientX, window.innerWidth - 160); const y = Math.max(e.clientY - 90, 8); setMenu({ visible: true, x, y }); } }}
        onMouseDown={e => { if (e.button !== 0) return; handleMouseDown(e); }}
        onMouseUp={cancelLongPress}
        onMouseLeave={cancelLongPress}
        onTouchStart={handleTouchStart}
        onTouchEnd={cancelLongPress}
        onTouchMove={cancelLongPress}
        // Empêche la sélection de texte pendant le long-press
        style={{ WebkitUserSelect: "none", userSelect: "none" }}
      >
        <div className={cn("flex gap-2 max-w-[85%] md:max-w-[70%]", isUser ? "flex-row-reverse" : "flex-row")}>
          {/* Avatar contact */}
          {showAvatar && !isUser && isLastInGroup && (
            <Avatar className="h-8 w-8 mt-auto shrink-0">
              <AvatarImage src={message.avatar} alt={message.senderName} />
              <AvatarFallback className="bg-primary text-primary-foreground text-xs font-medium">
                {getInitials(message.senderName || "")}
              </AvatarFallback>
            </Avatar>
          )}
          {showAvatar && !isUser && !isLastInGroup && <div className="w-8 shrink-0" />}

          <div className={cn("flex flex-col gap-0.5 min-w-0 w-full overflow-hidden", isUser ? "items-end" : "items-start")}>
            {/* Nom expéditeur */}
            {!isUser && isFirstInGroup && message.senderName && (
              <span className="text-xs font-medium text-primary ml-1 mb-0.5">
                {message.senderName}
              </span>
            )}

            {/* Aperçu réponse */}
            {message.replyTo && (
              <div className={cn(
                "text-xs px-3 py-1.5 rounded-lg mb-1 border-l-2 max-w-full",
                isUser
                  ? "bg-primary/15 border-primary/60 text-primary-foreground/80"
                  : "bg-muted border-primary/40 text-muted-foreground"
              )}>
                <span className="font-medium text-primary">{message.replyTo.senderName}</span>
                <p className="truncate">{message.replyTo.content}</p>
              </div>
            )}

            {/* Bulle principale */}
            <div className={cn(
              "relative break-words max-w-full overflow-hidden",
              isEmojiOnly
                ? "px-1 py-1 bg-transparent"
                : cn(
                    "px-4 py-2.5",
                    isUser
                      ? cn(
                          "bg-primary text-primary-foreground",
                          isFirstInGroup && isLastInGroup  && "rounded-2xl",
                          isFirstInGroup && !isLastInGroup && "rounded-2xl rounded-br-md",
                          !isFirstInGroup && isLastInGroup  && "rounded-2xl rounded-tr-md",
                          !isFirstInGroup && !isLastInGroup && "rounded-2xl rounded-r-md"
                        )
                      : cn(
                          "bg-muted text-foreground",
                          isFirstInGroup && isLastInGroup  && "rounded-2xl",
                          isFirstInGroup && !isLastInGroup && "rounded-2xl rounded-bl-md",
                          !isFirstInGroup && isLastInGroup  && "rounded-2xl rounded-tl-md",
                          !isFirstInGroup && !isLastInGroup && "rounded-2xl rounded-l-md"
                        )
                  )
            )}>
              <p className={cn(
                "leading-relaxed break-words overflow-hidden w-full",
                isEmojiOnly ? "text-4xl" : "text-sm whitespace-pre-wrap"
              )}>
                {message.content}
              </p>

              <div className={cn("flex items-center gap-1.5 mt-1", isUser ? "justify-end" : "justify-start")}>
                <span className={cn("text-[10px]", isUser ? "text-primary-foreground/60" : "text-muted-foreground")}>
                  {showTimestamp && formatTime(message.timestamp)}
                  {message.isEdited && " (modifié)"}
                </span>
                {isUser && <StatusIcon />}
              </div>
            </div>
          </div>

          {/* Avatar utilisateur */}
          {showAvatar && isUser && isLastInGroup && (
            <Avatar className="h-8 w-8 mt-auto shrink-0">
              <AvatarImage src={message.avatar} alt="You" />
              <AvatarFallback className="bg-primary text-primary-foreground text-xs font-medium">
                ME
              </AvatarFallback>
            </Avatar>
          )}
          {showAvatar && isUser && !isLastInGroup && <div className="w-8 shrink-0" />}
        </div>
      </div>
    </>
  );
}
