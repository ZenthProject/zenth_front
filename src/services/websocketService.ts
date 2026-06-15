/**
 * WebSocket Service - Handles real-time connection to DHT server
 * Supports notifications for friend requests, messages, etc.
 * Uses custom Tauri WebSocket with TLS bypass support
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

// ============================================================================
// Types
// ============================================================================

export interface WsAuthData {
  user_hash: string;
  session_token: string;
  ws_url: string;
}

/**
 * Notification types matching the protobuf enum
 */
export enum NotificationType {
  Unknown = 0,
  FriendRequestReceived = 1,
  FriendRequestAccepted = 2,
  FriendRequestRejected = 3,
  MessageReceived = 4,
}

/**
 * Friend request notification payload
 */
export interface FriendRequestNotification {
  requesterHashId: string;  // hex encoded
  preKeyBundle: Uint8Array;
  encryptedMessage: Uint8Array;
  timestamp: number;
}

/**
 * Friend response notification (for accepted/rejected)
 */
export interface FriendResponseNotification {
  responderHashId: string;  // hex encoded
  requesterHashId: string;  // hex encoded
  accepted: boolean;
  preKeyBundle: Uint8Array;
  timestamp: number;
}

/**
 * Message received notification
 */
export interface MessageNotification {
  senderHashId: string;  // hex encoded
  messageId: string;
  timestamp: number;
}

/**
 * Generic WebSocket notification
 */
export interface WsNotificationEvent {
  type: NotificationType;
  timestamp: number;
  payload: FriendRequestNotification | FriendResponseNotification | MessageNotification | null;
}

/**
 * Legacy incoming message type (for backwards compatibility)
 */
export interface IncomingMessage {
  type: 'message' | 'status' | 'error';
  senderId: string;
  messageId: string;
  content: string;
  timestamp: number;
}

// Handler types
export type MessageHandler = (message: IncomingMessage) => void;
export type ConnectionHandler = (connected: boolean) => void;
export type NotificationHandler = (notification: WsNotificationEvent) => void;
export type FriendRequestHandler = (notification: FriendRequestNotification) => void;
export type FriendResponseHandler = (notification: FriendResponseNotification, accepted: boolean) => void;

// ============================================================================
// Protobuf Parser (simplified varint/bytes parsing)
// ============================================================================

class ProtobufParser {
  private data: Uint8Array;
  private pos: number = 0;

  constructor(data: Uint8Array) {
    this.data = data;
  }

  hasMore(): boolean {
    return this.pos < this.data.length;
  }

  readVarint(): number {
    let result = 0;
    let shift = 0;
    while (this.pos < this.data.length) {
      const byte = this.data[this.pos++];
      result |= (byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) break;
      shift += 7;
    }
    return result;
  }

  readBytes(): Uint8Array {
    const length = this.readVarint();
    const bytes = this.data.slice(this.pos, this.pos + length);
    this.pos += length;
    return bytes;
  }

  readTag(): { fieldNumber: number; wireType: number } | null {
    if (!this.hasMore()) return null;
    const tag = this.readVarint();
    return {
      fieldNumber: tag >> 3,
      wireType: tag & 0x7,
    };
  }

  skip(wireType: number): void {
    switch (wireType) {
      case 0: // varint
        this.readVarint();
        break;
      case 2: // length-delimited
        this.readBytes();
        break;
      case 5: // 32-bit
        this.pos += 4;
        break;
      case 1: // 64-bit
        this.pos += 8;
        break;
    }
  }
}

// ============================================================================
// WebSocket Service
// ============================================================================

// Tauri event payload type
interface WsMessageEvent {
  type: string;
  data: number[] | null;
}

class WebSocketService {
  private connected = false;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;
  private isConnecting = false;
  private authData: WsAuthData | null = null;
  private credentials: { sessionToken: string } | null = null;
  private eventUnlisten: UnlistenFn | null = null;

  // Handler sets for different event types
  private messageHandlers: Set<MessageHandler> = new Set();
  private connectionHandlers: Set<ConnectionHandler> = new Set();
  private notificationHandlers: Set<NotificationHandler> = new Set();
  private friendRequestHandlers: Set<FriendRequestHandler> = new Set();
  private friendResponseHandlers: Set<FriendResponseHandler> = new Set();

  /**
   * Connect to the WebSocket server
   */
  async connect(sessionToken: string): Promise<boolean> {
    if (this.connected || this.isConnecting) {
      return true;
    }

    this.isConnecting = true;
    this.credentials = { sessionToken };

    try {
      this.authData = await invoke<WsAuthData>('get_ws_auth', { sessionToken });

      if (this.eventUnlisten) {
        this.eventUnlisten();
      }
      this.eventUnlisten = await listen<WsMessageEvent>('ws-message', (event) => {
        this.handleMessage({
          type: event.payload.type,
          data: event.payload.data ?? undefined,
        });
      });

      await invoke<string>('ws_connect', { url: this.authData.ws_url });

      const userHashBytes = this.hexToUint8Array(this.authData.user_hash);
      const sessionTokenBytes = this.hexToUint8Array(this.authData.session_token);

      const authMessage = new Uint8Array(userHashBytes.length + sessionTokenBytes.length);
      authMessage.set(userHashBytes, 0);
      authMessage.set(sessionTokenBytes, userHashBytes.length);

      await invoke('ws_send', { data: Array.from(authMessage) });

      this.connected = true;
      this.isConnecting = false;
      this.reconnectAttempts = 0;
      this.notifyConnectionChange(true);

      return true;
    } catch (error) {
      console.error('[WS] Connection failed:', error);
      this.isConnecting = false;
      this.connected = false;
      this.scheduleReconnect();
      return false;
    }
  }

  /**
   * Disconnect from the WebSocket server
   */
  async disconnect(): Promise<void> {
    this.credentials = null;
    if (this.connected) {
      try {
        await invoke('ws_disconnect');
      } catch (error) {
        console.error('[WS] Disconnect error:', error);
      }
      this.connected = false;
    }
    if (this.eventUnlisten) {
      this.eventUnlisten();
      this.eventUnlisten = null;
    }
    this.notifyConnectionChange(false);
  }

  /**
   * Check if connected
   */
  isConnected(): boolean {
    return this.connected;
  }

  // ============================================================================
  // Event Subscription Methods
  // ============================================================================

  /**
   * Subscribe to legacy message events
   */
  onMessage(handler: MessageHandler): () => void {
    this.messageHandlers.add(handler);
    return () => this.messageHandlers.delete(handler);
  }

  /**
   * Subscribe to connection status changes
   */
  onConnectionChange(handler: ConnectionHandler): () => void {
    this.connectionHandlers.add(handler);
    return () => this.connectionHandlers.delete(handler);
  }

  /**
   * Subscribe to all WebSocket notifications
   */
  onNotification(handler: NotificationHandler): () => void {
    this.notificationHandlers.add(handler);
    return () => this.notificationHandlers.delete(handler);
  }

  /**
   * Subscribe specifically to friend request received events
   */
  onFriendRequest(handler: FriendRequestHandler): () => void {
    this.friendRequestHandlers.add(handler);
    return () => this.friendRequestHandlers.delete(handler);
  }

  /**
   * Subscribe to friend request responses (accepted/rejected)
   */
  onFriendResponse(handler: FriendResponseHandler): () => void {
    this.friendResponseHandlers.add(handler);
    return () => this.friendResponseHandlers.delete(handler);
  }

  // ============================================================================
  // Message Handling
  // ============================================================================

  /**
   * Handle incoming WebSocket message
   */
  private handleMessage(message: { type: string; data?: number[] }): void {
    if (message.type === 'Binary' && message.data && Array.isArray(message.data)) {
      try {
        const bytes = new Uint8Array(message.data);

        const notification = this.parseWsNotification(bytes);

        if (notification) {
          this.dispatchNotification(notification);
        }
      } catch (error) {
        console.error('[WS] Failed to parse message:', error);
      }
    } else if (message.type === 'Close') {
      this.connected = false;
      this.notifyConnectionChange(false);
      this.scheduleReconnect();
    } else if (message.type === 'Error') {
      this.connected = false;
      this.notifyConnectionChange(false);
      this.scheduleReconnect();
    }
  }

  /**
   * Parse WsNotification protobuf message
   *
   * WsNotification {
   *   notification_type: i32 (field 1)
   *   timestamp: u64 (field 2)
   *   payload: bytes (field 3)
   * }
   */
  private parseWsNotification(bytes: Uint8Array): WsNotificationEvent | null {
    const parser = new ProtobufParser(bytes);

    let notificationType: NotificationType = NotificationType.Unknown;
    let timestamp = 0;
    let payloadBytes: Uint8Array = new Uint8Array(0);

    while (parser.hasMore()) {
      const tag = parser.readTag();
      if (!tag) break;

      switch (tag.fieldNumber) {
        case 1: // notification_type (varint)
          notificationType = parser.readVarint() as NotificationType;
          break;
        case 2: // timestamp (varint)
          timestamp = parser.readVarint();
          break;
        case 3: // payload (bytes)
          payloadBytes = parser.readBytes();
          break;
        default:
          parser.skip(tag.wireType);
      }
    }

    // Parse the nested payload based on notification type
    let payload: WsNotificationEvent['payload'] = null;

    switch (notificationType) {
      case NotificationType.FriendRequestReceived:
        payload = this.parseFriendRequestNotification(payloadBytes);
        break;
      case NotificationType.FriendRequestAccepted:
      case NotificationType.FriendRequestRejected:
        payload = this.parseFriendResponseNotification(payloadBytes);
        break;
      case NotificationType.MessageReceived:
        payload = this.parseMessageNotification(payloadBytes);
        break;
    }

    return {
      type: notificationType,
      timestamp,
      payload,
    };
  }

  /**
   * Parse FriendRequestNotification
   *
   * FriendRequestNotification {
   *   requester_hash_id: bytes (field 1)
   *   pre_key_bundle: bytes (field 2)
   *   encrypted_message: bytes (field 3)
   *   timestamp: u64 (field 4)
   * }
   */
  private parseFriendRequestNotification(bytes: Uint8Array): FriendRequestNotification | null {
    if (bytes.length === 0) return null;

    const parser = new ProtobufParser(bytes);
    let requesterHashId = '';
    let preKeyBundle = new Uint8Array(0);
    let encryptedMessage = new Uint8Array(0);
    let timestamp = 0;

    while (parser.hasMore()) {
      const tag = parser.readTag();
      if (!tag) break;

      switch (tag.fieldNumber) {
        case 1:
          requesterHashId = this.uint8ArrayToHex(parser.readBytes());
          break;
        case 2:
          preKeyBundle = parser.readBytes();
          break;
        case 3:
          encryptedMessage = parser.readBytes();
          break;
        case 4:
          timestamp = parser.readVarint();
          break;
        default:
          parser.skip(tag.wireType);
      }
    }

    return { requesterHashId, preKeyBundle, encryptedMessage, timestamp };
  }

  /**
   * Parse FriendResponse (for accepted/rejected notifications)
   *
   * FriendResponse {
   *   responder_hash_id: bytes (field 1)
   *   requester_hash_id: bytes (field 2)
   *   accepted: bool (field 3)
   *   pre_key_bundle: bytes (field 4)
   *   dilithium_signature: bytes (field 5)
   *   timestamp: u64 (field 6)
   * }
   */
  private parseFriendResponseNotification(bytes: Uint8Array): FriendResponseNotification | null {
    if (bytes.length === 0) return null;

    const parser = new ProtobufParser(bytes);
    let responderHashId = '';
    let requesterHashId = '';
    let accepted = false;
    let preKeyBundle = new Uint8Array(0);
    let timestamp = 0;

    while (parser.hasMore()) {
      const tag = parser.readTag();
      if (!tag) break;

      switch (tag.fieldNumber) {
        case 1:
          responderHashId = this.uint8ArrayToHex(parser.readBytes());
          break;
        case 2:
          requesterHashId = this.uint8ArrayToHex(parser.readBytes());
          break;
        case 3:
          accepted = parser.readVarint() !== 0;
          break;
        case 4:
          preKeyBundle = parser.readBytes();
          break;
        case 5:
          // dilithium_signature - skip
          parser.readBytes();
          break;
        case 6:
          timestamp = parser.readVarint();
          break;
        default:
          parser.skip(tag.wireType);
      }
    }

    return { responderHashId, requesterHashId, accepted, preKeyBundle, timestamp };
  }

  /**
   * Parse MessageNotification (for incoming message notifications)
   *
   * MessageNotification {
   *   sender_hash_id: bytes (field 1)
   *   message_id: bytes (field 2)
   *   timestamp: u64 (field 3)
   * }
   */
  private parseMessageNotification(bytes: Uint8Array): MessageNotification | null {
    if (bytes.length === 0) return null;

    const parser = new ProtobufParser(bytes);
    let senderHashId = '';
    let messageId = '';
    let timestamp = 0;

    while (parser.hasMore()) {
      const tag = parser.readTag();
      if (!tag) break;

      switch (tag.fieldNumber) {
        case 1:
          senderHashId = this.uint8ArrayToHex(parser.readBytes());
          break;
        case 2:
          // message_id is bytes in protobuf, convert to hex string
          messageId = this.uint8ArrayToHex(parser.readBytes());
          break;
        case 3:
          timestamp = parser.readVarint();
          break;
        default:
          parser.skip(tag.wireType);
      }
    }

    return { senderHashId, messageId, timestamp };
  }

  /**
   * Dispatch notification to registered handlers
   */
  private dispatchNotification(notification: WsNotificationEvent): void {
    // Notify generic handlers
    this.notificationHandlers.forEach(handler => {
      try {
        handler(notification);
      } catch (error) {
        console.error('[WS] Notification handler error:', error);
      }
    });

    // Dispatch to specific handlers based on type
    switch (notification.type) {
      case NotificationType.FriendRequestReceived:
        if (notification.payload) {
          const payload = notification.payload as FriendRequestNotification;
          this.friendRequestHandlers.forEach(handler => {
            try {
              handler(payload);
            } catch (error) {
              console.error('[WS] Friend request handler error:', error);
            }
          });
        }
        break;

      case NotificationType.FriendRequestAccepted:
      case NotificationType.FriendRequestRejected:
        if (notification.payload) {
          const payload = notification.payload as FriendResponseNotification;
          const accepted = notification.type === NotificationType.FriendRequestAccepted;
          this.friendResponseHandlers.forEach(handler => {
            try {
              handler(payload, accepted);
            } catch (error) {
              console.error('[WS] Friend response handler error:', error);
            }
          });
        }
        break;

      case NotificationType.MessageReceived:
        // Legacy message handler support
        if (notification.payload) {
          const msg = notification.payload as MessageNotification;
          const legacyMessage: IncomingMessage = {
            type: 'message',
            senderId: msg.senderHashId,
            messageId: msg.messageId,
            content: '',
            timestamp: msg.timestamp,
          };
          this.messageHandlers.forEach(handler => {
            try {
              handler(legacyMessage);
            } catch (error) {
              console.error('[WS] Message handler error:', error);
            }
          });
        }
        break;
    }
  }

  // ============================================================================
  // Utility Methods
  // ============================================================================

  /**
   * Schedule reconnection attempt
   */
  private scheduleReconnect(): void {
    if (!this.credentials) {
      return;
    }

    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      return;
    }

    this.reconnectAttempts++;
    const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);

    setTimeout(() => {
      if (this.credentials) {
        this.connect(this.credentials.sessionToken);
      }
    }, delay);
  }

  /**
   * Notify connection change handlers
   */
  private notifyConnectionChange(connected: boolean): void {
    this.connectionHandlers.forEach(handler => {
      try {
        handler(connected);
      } catch (error) {
        console.error('[WS] Connection handler error:', error);
      }
    });
  }

  /**
   * Convert hex string to Uint8Array
   */
  private hexToUint8Array(hex: string): Uint8Array {
    const bytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < bytes.length; i++) {
      bytes[i] = parseInt(hex.substr(i * 2, 2), 16);
    }
    return bytes;
  }

  /**
   * Convert Uint8Array to hex string
   */
  private uint8ArrayToHex(bytes: Uint8Array): string {
    return Array.from(bytes)
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');
  }
}

// Singleton instance
export const wsService = new WebSocketService();
