import { createContext, useContext, useEffect, useState, useCallback, ReactNode } from 'react';
import {
  wsService,
  IncomingMessage,
  FriendRequestHandler,
  FriendResponseHandler,
} from '@/services/websocketService';
import { useAuth } from '@/hooks/use-auth';

interface WebSocketContextType {
  isConnected: boolean;
  lastMessage: IncomingMessage | null;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  // Friend request event subscriptions
  onFriendRequest: (handler: FriendRequestHandler) => () => void;
  onFriendResponse: (handler: FriendResponseHandler) => () => void;
}

const WebSocketContext = createContext<WebSocketContextType | undefined>(undefined);

export const WebSocketProvider = ({ children }: { children: ReactNode }) => {
  const { sessionToken, isAuthenticated } = useAuth();
  const [isConnected, setIsConnected] = useState(false);
  const [lastMessage, setLastMessage] = useState<IncomingMessage | null>(null);

  // Connect when authenticated
  useEffect(() => {
    if (isAuthenticated && sessionToken) {
      wsService.connect(sessionToken);
    } else {
      wsService.disconnect();
    }

    return () => {
      wsService.disconnect();
    };
  }, [isAuthenticated, sessionToken]);

  // Subscribe to connection status changes
  useEffect(() => {
    const unsubscribe = wsService.onConnectionChange((connected) => {
      setIsConnected(connected);
    });

    return unsubscribe;
  }, []);

  // Subscribe to incoming messages
  useEffect(() => {
    const unsubscribe = wsService.onMessage((message) => {
      setLastMessage(message);
    });

    return unsubscribe;
  }, []);

  const connect = useCallback(async () => {
    if (sessionToken) {
      await wsService.connect(sessionToken);
    }
  }, [sessionToken]);

  const disconnect = useCallback(async () => {
    await wsService.disconnect();
  }, []);

  // Expose friend request subscription
  const onFriendRequest = useCallback((handler: FriendRequestHandler) => {
    return wsService.onFriendRequest(handler);
  }, []);

  // Expose friend response subscription
  const onFriendResponse = useCallback((handler: FriendResponseHandler) => {
    return wsService.onFriendResponse(handler);
  }, []);

  return (
    <WebSocketContext.Provider value={{
      isConnected,
      lastMessage,
      connect,
      disconnect,
      onFriendRequest,
      onFriendResponse,
    }}>
      {children}
    </WebSocketContext.Provider>
  );
};

export const useWebSocket = () => {
  const context = useContext(WebSocketContext);
  if (!context) {
    throw new Error('useWebSocket must be used within WebSocketProvider');
  }
  return context;
};
