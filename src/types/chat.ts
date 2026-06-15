// TODO: Chat and Message types
//
// This file defines types for chat functionality.

export interface Message {
  id: string;
  friendId: string;
  content: string;
  encryptedContent?: string;
  timestamp: Date;
  sender: 'user' | 'friend';
  status: 'sending' | 'sent' | 'delivered' | 'read' | 'error';
  messageType: 'text' | 'image' | 'video' | 'audio' | 'file';
  fileName?: string;
  fileSize?: number;
  fileMimeType?: string;
  thumbnail?: string;
}

export interface Conversation {
  friendId: string;
  friendUsername: string;
  lastMessage?: Message;
  unreadCount: number;
  messages: Message[];
}

export interface FileAttachment {
  name: string;
  size: number;
  mimeType: string;
  data: Uint8Array;
  thumbnail?: string;
}

export interface ChatSession {
  friendId: string;
  ratchetState: any; // Double Ratchet state
  sessionKey: string;
}
