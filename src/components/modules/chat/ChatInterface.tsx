import { useRef, useLayoutEffect, useState, useMemo } from "react";
import { cn } from "@/lib/utils";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ChatHeader } from "./ChatHeader";
import { ChatInput } from "./ChatInput";
import { MessageBubble, Message } from "./MessageBubble";
import { EmptyState } from "./EmptyState";
import { DateSeparator } from "./DateSeparator";

export type { Message };

export interface ChatInterfaceProps {
  messages: Message[];
  onSendMessage: (message: string) => void;
  onMessageAction?: (messageId: string, action: string) => void;
  contactName: string;
  contactAvatar?: string;
  isOnline?: boolean;
  lastSeen?: string;
  isEncrypted?: boolean;
  isVerified?: boolean;
  disabled?: boolean;
  placeholder?: string;
  className?: string;
  onBack?: () => void;
  onCall?: () => void;
  onVideoCall?: () => void;
  onViewProfile?: () => void;
  showHeader?: boolean;
}

export function ChatInterface({
  messages,
  onSendMessage,
  onMessageAction,
  contactName,
  contactAvatar,
  isOnline = false,
  lastSeen,
  isEncrypted = true,
  isVerified = false,
  disabled = false,
  placeholder,
  className,
  onBack,
  onCall,
  onVideoCall,
  onViewProfile,
  showHeader = true,
}: ChatInterfaceProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [replyingTo, setReplyingTo] = useState<Message | null>(null);
  const [ephemeralMode, setEphemeralMode] = useState(false);

  // Scroll instantané en bas
  useLayoutEffect(() => {
    if (scrollRef.current) {
      const viewport = scrollRef.current.querySelector(
        "[data-radix-scroll-area-viewport]"
      ) as HTMLElement | null;
      if (viewport) {
        viewport.scrollTo({ top: viewport.scrollHeight, behavior: "instant" as ScrollBehavior });
      }
    }
  }, [messages]);

  // Group messages by date and determine first/last in consecutive same-sender groups
  const processedMessages = useMemo(() => {
    const result: Array<{
      type: "date" | "message";
      date?: Date;
      message?: Message;
      isFirstInGroup?: boolean;
      isLastInGroup?: boolean;
    }> = [];

    let lastDate: string | null = null;
    let lastSender: string | null = null;

    messages.forEach((message, index) => {
      const messageDate = new Date(message.timestamp).toDateString();
      const nextMessage = messages[index + 1];
      const prevMessage = messages[index - 1];

      // Add date separator if new day
      if (messageDate !== lastDate) {
        result.push({ type: "date", date: new Date(message.timestamp) });
        lastDate = messageDate;
        lastSender = null;
      }

      // Determine if first/last in group
      const isFirstInGroup =
        lastSender !== message.sender ||
        (prevMessage &&
          new Date(prevMessage.timestamp).toDateString() !== messageDate);

      const isLastInGroup =
        !nextMessage ||
        nextMessage.sender !== message.sender ||
        new Date(nextMessage.timestamp).toDateString() !== messageDate;

      result.push({
        type: "message",
        message,
        isFirstInGroup,
        isLastInGroup,
      });

      lastSender = message.sender;
    });

    return result;
  }, [messages]);

  const handleReply = (message: Message) => {
    setReplyingTo(message);
  };

  const handleCancelReply = () => {
    setReplyingTo(null);
  };

  const handleSend = (content: string) => {
    onSendMessage(content);
    setReplyingTo(null);
  };

  return (
    <div
      className={cn(
        "flex flex-col h-full bg-gray-950 overflow-hidden",
        className
      )}
    >
      {/* Header */}
      {showHeader && (
        <ChatHeader
          contactName={contactName}
          contactAvatar={contactAvatar}
          isOnline={isOnline}
          lastSeen={lastSeen}
          isEncrypted={isEncrypted}
          isVerified={isVerified}
          onBack={onBack}
          onCall={onCall}
          onVideoCall={onVideoCall}
          onViewProfile={onViewProfile}
        />
      )}

      {/* Messages Area */}
      <div className="flex-1 min-h-0 relative">
        {/* Background Pattern */}
        <div
          className="absolute inset-0 opacity-[0.02]"
          style={{
            backgroundImage: `url("data:image/svg+xml,%3Csvg width='60' height='60' viewBox='0 0 60 60' xmlns='http://www.w3.org/2000/svg'%3E%3Cg fill='none' fill-rule='evenodd'%3E%3Cg fill='%23ffffff' fill-opacity='1'%3E%3Cpath d='M36 34v-4h-2v4h-4v2h4v4h2v-4h4v-2h-4zm0-30V0h-2v4h-4v2h4v4h2V6h4V4h-4zM6 34v-4H4v4H0v2h4v4h2v-4h4v-2H6zM6 4V0H4v4H0v2h4v4h2V6h4V4H6z'/%3E%3C/g%3E%3C/g%3E%3C/svg%3E")`,
          }}
        />

        <ScrollArea ref={scrollRef} className="h-full">
          <div className="px-4 py-2 min-h-full flex flex-col">
            {messages.length === 0 ? (
              <EmptyState contactName={contactName} />
            ) : (
              <div className="space-y-1 mt-auto">
                {processedMessages.map((item, index) => {
                  if (item.type === "date" && item.date) {
                    return <DateSeparator key={`date-${index}`} date={item.date} />;
                  }

                  if (item.type === "message" && item.message) {
                    return (
                      <MessageBubble
                        key={item.message.id}
                        message={item.message}
                        isFirstInGroup={item.isFirstInGroup}
                        isLastInGroup={item.isLastInGroup}
                        onAction={onMessageAction}
                        onReply={handleReply}
                      />
                    );
                  }

                  return null;
                })}
              </div>
            )}
          </div>
        </ScrollArea>
      </div>

      {/* Input Area */}
      <ChatInput
        onSend={handleSend}
        disabled={disabled}
        placeholder={placeholder}
        replyingTo={replyingTo}
        onCancelReply={handleCancelReply}
        isEncrypted={isEncrypted}
        ephemeralMode={ephemeralMode}
        onToggleEphemeral={() => setEphemeralMode(!ephemeralMode)}
      />
    </div>
  );
}

// Legacy export for backward compatibility
export { ChatInterface as Chat };
