import { useState, useEffect } from "react";
import { Shield, WifiOff, Globe } from "lucide-react";
import { cn } from "@/lib/utils";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

type NetworkType = "tor" | "i2p" | "lokinet" | "direct" | "offline";

interface NetworkState {
  type: NetworkType;
  connected: boolean;
  latency?: number;
}

const networkConfig: Record<NetworkType, { label: string; color: string; icon: typeof Shield }> = {
  tor: { label: "Tor", color: "text-purple-500", icon: Shield },
  i2p: { label: "I2P", color: "text-green-500", icon: Shield },
  lokinet: { label: "Lokinet", color: "text-blue-500", icon: Shield },
  direct: { label: "Direct", color: "text-yellow-500", icon: Globe },
  offline: { label: "Hors ligne", color: "text-destructive", icon: WifiOff },
};

export function NetworkIndicator() {
  const [network, _setNetwork] = useState<NetworkState>({
    type: "tor",
    connected: true,
  });

  // TODO: Connect to actual network status from Tauri backend
  useEffect(() => {
    // Placeholder for network status polling
    const checkNetwork = async () => {
      // In the future, call invoke('get_network_status') here
      // For now, default to Tor connected
    };

    checkNetwork();
    const interval = setInterval(checkNetwork, 30000);
    return () => clearInterval(interval);
  }, []);

  const config = networkConfig[network.type];
  const Icon = network.connected ? config.icon : WifiOff;

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            className={cn(
              "flex items-center gap-1.5 px-2.5 py-1.5 rounded-md",
              "bg-secondary/50 hover:bg-secondary transition-colors",
              "text-sm font-medium"
            )}
          >
            <span className="relative flex h-2 w-2">
              <span
                className={cn(
                  "absolute inline-flex h-full w-full rounded-full opacity-75",
                  network.connected ? "animate-ping bg-success" : "bg-destructive"
                )}
              />
              <span
                className={cn(
                  "relative inline-flex h-2 w-2 rounded-full",
                  network.connected ? "bg-success" : "bg-destructive"
                )}
              />
            </span>
            <Icon className={cn("h-4 w-4", config.color)} />
            <span className={cn("hidden sm:inline", config.color)}>
              {config.label}
            </span>
          </button>
        </TooltipTrigger>
        <TooltipContent>
          <div className="text-sm">
            <p className="font-medium">
              {network.connected ? "Connecte via " : "Deconnecte de "}
              {config.label}
            </p>
            {network.latency && (
              <p className="text-muted-foreground">
                Latence: {network.latency}ms
              </p>
            )}
            <p className="text-xs text-muted-foreground mt-1">
              {network.type === "tor" && "Anonymat maximum"}
              {network.type === "i2p" && "Reseau decentralise"}
              {network.type === "lokinet" && "Routage en oignon"}
              {network.type === "direct" && "Connexion non anonyme"}
              {network.type === "offline" && "Aucune connexion"}
            </p>
          </div>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}

// Hook for components that need to know network status
export function useNetworkStatus() {
  const [network] = useState<NetworkState>({
    type: "tor",
    connected: true,
  });

  // TODO: Implement actual network status checking
  return network;
}
