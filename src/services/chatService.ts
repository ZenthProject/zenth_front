/**
 * Chat Service - Handles all message-related operations via Tauri commands
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * Message information returned from backend
 */
export interface MessageInfo {
  id: number;
  message_id: string;
  friend_id: number;
  content: string;
  is_outgoing: boolean;
  timestamp: number;
  status: 'pending' | 'sent' | 'delivered' | 'read' | 'failed';
  delivered_at: number | null;
  read_at: number | null;
  message_type: string;
  file_name: string | null;
  file_mime: string | null;
  file_data: string | null; // base64 encoded
  reply_to_id: string | null;
}

/**
 * Result of message sync operation
 */
export interface MessageSyncResult {
  new_messages: number;
  errors: string[];
  updated_friend_ids: number[];
}

/**
 * Parameters for sending a message
 */
export interface SendMessageParams {
  sessionToken: string;
  friendId: number;
  content: string;
  fileData?: number[];
  fileName?: string;
  fileMime?: string;
  replyToId?: string;
}

/**
 * Parameters for getting messages
 */
export interface GetMessagesParams {
  sessionToken: string;
  friendId: number;
  limit?: number;
  offset?: number;
}

/**
 * Parameters for auth-only operations
 */
export interface AuthParams {
  sessionToken: string;
}

/**
 * Parameters for marking message as read
 */
export interface MarkReadParams {
  sessionToken: string;
  messageId: string;
}

export class ChatService {

  static async sendMessage(params: SendMessageParams): Promise<MessageInfo> {
    try {
      const result = await invoke<MessageInfo>('send_message', {
        sessionToken: params.sessionToken,
        friendId: params.friendId,
        content: params.content,
        fileData: params.fileData ?? null,
        fileName: params.fileName ?? null,
        fileMime: params.fileMime ?? null,
        replyToId: params.replyToId ?? null,
      });
      return result;
    } catch (error) {
      console.error('[ChatService] send_message error:', error);
      throw error;
    }
  }

  /**
   * Get messages for a conversation
   */
  static async getMessages(params: GetMessagesParams): Promise<MessageInfo[]> {
    try {
      const result = await invoke<MessageInfo[]>('get_messages', {
        sessionToken: params.sessionToken,
        friendId: params.friendId,
        limit: params.limit,
        offset: params.offset,
      });
      return result;
    } catch (error) {
      console.error('[ChatService] getMessages error:', error);
      throw error;
    }
  }

  /**
   * Sync messages with the server (fetch new incoming messages)
   */
  static async syncMessages(params: AuthParams): Promise<MessageSyncResult> {
    return await invoke<MessageSyncResult>('sync_messages', {
      sessionToken: params.sessionToken,
    });
  }

  /**
   * Mark a message as read
   */
  static async markAsRead(params: MarkReadParams): Promise<void> {
    await invoke('mark_message_read', {
      sessionToken: params.sessionToken,
      messageId: params.messageId,
    });
  }
}
