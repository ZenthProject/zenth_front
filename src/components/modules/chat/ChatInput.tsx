import { useState, useRef, useEffect, KeyboardEvent } from "react";
import {
  Send,
  Paperclip,
  Smile,
  Mic,
  X,
  Image as ImageIcon,
  File,
  Timer,
  Lock,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Message } from "./MessageBubble";

interface ChatInputProps {
  onSend: (message: string) => void;
  disabled?: boolean;
  placeholder?: string;
  replyingTo?: Message | null;
  onCancelReply?: () => void;
  isEncrypted?: boolean;
  ephemeralMode?: boolean;
  onToggleEphemeral?: () => void;
  onAttachFile?: () => void;
  onAttachImage?: () => void;
  onVoiceMessage?: () => void;
}

export function ChatInput({
  onSend,
  disabled = false,
  placeholder = "Ecrivez un message...",
  replyingTo,
  onCancelReply,
  isEncrypted = true,
  ephemeralMode = false,
  onToggleEphemeral,
  onAttachFile,
  onAttachImage,
  onVoiceMessage,
}: ChatInputProps) {
  const [message, setMessage] = useState("");
  const [isFocused, setIsFocused] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 150)}px`;
    }
  }, [message]);

  const handleSend = () => {
    if (message.trim() && !disabled) {
      onSend(message.trim());
      setMessage("");
      if (textareaRef.current) {
        textareaRef.current.style.height = "auto";
      }
    }
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="p-3 bg-gray-900/95 backdrop-blur-sm border-t border-gray-800">
      {/* Reply Preview */}
      {replyingTo && (
        <div className="flex items-center gap-2 mb-2 px-3 py-2 bg-gray-800/50 rounded-lg border-l-2 border-indigo-500">
          <div className="flex-1 min-w-0">
            <span className="text-xs font-medium text-indigo-400">
              Reponse a {replyingTo.senderName}
            </span>
            <p className="text-xs text-gray-400 truncate">{replyingTo.content}</p>
          </div>
          <Button
            variant="ghost"
            size="icon"
            onClick={onCancelReply}
            className="h-6 w-6 text-gray-500 hover:text-gray-300 hover:bg-gray-700"
          >
            <X className="h-4 w-4" />
          </Button>
        </div>
      )}

      {/* Input Area */}
      <div
        className={cn(
          "flex items-end gap-2 bg-gray-800 rounded-2xl px-3 py-2 transition-all duration-200",
          isFocused && "ring-2 ring-indigo-500/50"
        )}
      >
        {/* Attachment Button */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              disabled={disabled}
              className="h-9 w-9 shrink-0 text-gray-400 hover:text-gray-200 hover:bg-gray-700"
            >
              <Paperclip className="h-5 w-5" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="start"
            className="bg-gray-800 border-gray-700 w-48"
          >
            <DropdownMenuItem
              onClick={onAttachImage}
              className="text-gray-300 focus:bg-gray-700 focus:text-gray-100"
            >
              <ImageIcon className="h-4 w-4 mr-2 text-indigo-400" />
              Photo ou video
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={onAttachFile}
              className="text-gray-300 focus:bg-gray-700 focus:text-gray-100"
            >
              <File className="h-4 w-4 mr-2 text-purple-400" />
              Fichier
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        {/* Textarea */}
        <div className="flex-1 min-w-0">
          <textarea
            ref={textareaRef}
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            onKeyDown={handleKeyDown}
            onFocus={() => setIsFocused(true)}
            onBlur={() => setIsFocused(false)}
            placeholder={placeholder}
            disabled={disabled}
            rows={1}
            className={cn(
              "w-full bg-transparent text-gray-100 placeholder-gray-500 resize-none",
              "focus:outline-none text-sm leading-relaxed py-1.5",
              "scrollbar-thin scrollbar-thumb-gray-700 scrollbar-track-transparent"
            )}
          />
        </div>

        {/* Right Actions */}
        <div className="flex items-center gap-1 shrink-0">
          {/* Ephemeral Toggle */}
          {onToggleEphemeral && (
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={onToggleEphemeral}
                    disabled={disabled}
                    className={cn(
                      "h-9 w-9",
                      ephemeralMode
                        ? "text-amber-400 hover:text-amber-300 hover:bg-amber-900/20"
                        : "text-gray-400 hover:text-gray-200 hover:bg-gray-700"
                    )}
                  >
                    <Timer className="h-5 w-5" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent className="bg-gray-800 border-gray-700">
                  <p className="text-xs">
                    {ephemeralMode ? "Messages ephemeres actifs" : "Activer messages ephemeres"}
                  </p>
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
          )}

          {/* Emoji Button */}
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  disabled={disabled}
                  className="h-9 w-9 text-gray-400 hover:text-gray-200 hover:bg-gray-700"
                >
                  <Smile className="h-5 w-5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent className="bg-gray-800 border-gray-700">
                <p className="text-xs">Emoji</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>

          {/* Send or Voice Button */}
          {message.trim() ? (
            <Button
              onClick={handleSend}
              disabled={disabled}
              size="icon"
              className="h-9 w-9 bg-indigo-600 hover:bg-indigo-500 text-white rounded-full transition-all duration-200 hover:scale-105"
            >
              <Send className="h-4 w-4" />
            </Button>
          ) : onVoiceMessage ? (
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={onVoiceMessage}
                    disabled={disabled}
                    className="h-9 w-9 text-gray-400 hover:text-gray-200 hover:bg-gray-700"
                  >
                    <Mic className="h-5 w-5" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent className="bg-gray-800 border-gray-700">
                  <p className="text-xs">Message vocal</p>
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
          ) : (
            <Button
              disabled
              size="icon"
              className="h-9 w-9 bg-gray-700 text-gray-500 rounded-full cursor-not-allowed"
            >
              <Send className="h-4 w-4" />
            </Button>
          )}
        </div>
      </div>

      {/* Security Indicator */}
      {isEncrypted && (
        <div className="flex items-center justify-center gap-1.5 mt-2 text-gray-500">
          <Lock className="h-3 w-3" />
          <span className="text-[10px]">Chiffrement de bout en bout</span>
        </div>
      )}
    </div>
  );
}
