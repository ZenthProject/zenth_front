import { Shield, Phone, Video, MoreVertical, ChevronLeft, Lock, ShieldCheck } from "lucide-react";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface ChatHeaderProps {
  contactName: string;
  contactAvatar?: string;
  isOnline?: boolean;
  lastSeen?: string;
  isEncrypted?: boolean;
  isVerified?: boolean;
  onBack?: () => void;
  onCall?: () => void;
  onVideoCall?: () => void;
  onViewProfile?: () => void;
  onMuteNotifications?: () => void;
  onClearChat?: () => void;
  onBlockContact?: () => void;
}

export function ChatHeader({
  contactName,
  contactAvatar,
  isOnline = false,
  lastSeen,
  isEncrypted = true,
  isVerified = false,
  onBack,
  onCall,
  onVideoCall,
  onViewProfile,
  onMuteNotifications,
  onClearChat,
  onBlockContact,
}: ChatHeaderProps) {
  const getInitials = (name: string) => {
    return name
      .split(" ")
      .map((n) => n[0])
      .join("")
      .toUpperCase()
      .slice(0, 2);
  };

  return (
    <div className="flex items-center justify-between px-4 py-3 bg-gray-900/95 backdrop-blur-sm border-b border-gray-800">
      <div className="flex items-center gap-3">
        {onBack && (
          <Button
            variant="ghost"
            size="icon"
            onClick={onBack}
            className="text-gray-400 hover:text-gray-100 hover:bg-gray-800 -ml-2 md:hidden"
          >
            <ChevronLeft className="h-5 w-5" />
          </Button>
        )}

        <div className="relative">
          <Avatar className="h-10 w-10 ring-2 ring-gray-700">
            <AvatarImage src={contactAvatar} alt={contactName} />
            <AvatarFallback className="bg-gradient-to-br from-indigo-500 to-purple-600 text-white font-medium">
              {getInitials(contactName)}
            </AvatarFallback>
          </Avatar>
          {isOnline && (
            <span className="absolute bottom-0 right-0 h-3 w-3 bg-emerald-500 border-2 border-gray-900 rounded-full" />
          )}
        </div>

        <div className="flex flex-col">
          <div className="flex items-center gap-2">
            <span className="font-semibold text-gray-100">{contactName}</span>
            {isVerified && (
              <TooltipProvider>
                <Tooltip>
                  <TooltipTrigger>
                    <ShieldCheck className="h-4 w-4 text-emerald-500" />
                  </TooltipTrigger>
                  <TooltipContent className="bg-gray-800 border-gray-700">
                    <p className="text-xs">Identite verifiee</p>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
            )}
          </div>
          <div className="flex items-center gap-2">
            {isEncrypted && (
              <div className="flex items-center gap-1 text-emerald-500">
                <Lock className="h-3 w-3" />
                <span className="text-xs">E2E</span>
              </div>
            )}
            <span className="text-xs text-gray-500">
              {isOnline ? "En ligne" : lastSeen || "Hors ligne"}
            </span>
          </div>
        </div>
      </div>

      <div className="flex items-center gap-1">
        {onCall && (
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={onCall}
                  className="text-gray-400 hover:text-gray-100 hover:bg-gray-800"
                >
                  <Phone className="h-5 w-5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent className="bg-gray-800 border-gray-700">
                <p className="text-xs">Appel audio</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        )}

        {onVideoCall && (
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={onVideoCall}
                  className="text-gray-400 hover:text-gray-100 hover:bg-gray-800"
                >
                  <Video className="h-5 w-5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent className="bg-gray-800 border-gray-700">
                <p className="text-xs">Appel video</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        )}

        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="text-gray-400 hover:text-gray-100 hover:bg-gray-800"
              >
                <Shield className="h-5 w-5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent className="bg-gray-800 border-gray-700">
              <p className="text-xs">Verifier la securite</p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="text-gray-400 hover:text-gray-100 hover:bg-gray-800"
            >
              <MoreVertical className="h-5 w-5" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="end"
            className="bg-gray-800 border-gray-700 w-48"
          >
            {onViewProfile && (
              <DropdownMenuItem
                onClick={onViewProfile}
                className="text-gray-300 focus:bg-gray-700 focus:text-gray-100"
              >
                Voir le profil
              </DropdownMenuItem>
            )}
            {onMuteNotifications && (
              <DropdownMenuItem
                onClick={onMuteNotifications}
                className="text-gray-300 focus:bg-gray-700 focus:text-gray-100"
              >
                Couper les notifications
              </DropdownMenuItem>
            )}
            <DropdownMenuSeparator className="bg-gray-700" />
            {onClearChat && (
              <DropdownMenuItem
                onClick={onClearChat}
                className="text-gray-300 focus:bg-gray-700 focus:text-gray-100"
              >
                Effacer la conversation
              </DropdownMenuItem>
            )}
            {onBlockContact && (
              <DropdownMenuItem
                onClick={onBlockContact}
                className="text-red-400 focus:bg-red-900/20 focus:text-red-400"
              >
                Bloquer le contact
              </DropdownMenuItem>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
}
