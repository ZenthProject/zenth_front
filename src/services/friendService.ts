/**
 * Friend Service - Handles all friend-related operations via Tauri commands
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  UserPublicInfo,
  FriendInfo,
  PendingRequest,
  SendFriendRequestParams,
  FriendRequestResponseParams,
  ListParams,
  SearchUserParams,
  RemoveFriendParams,
} from '@/types/friends';

export class FriendService {
  /**
   * Search for a user by their username hash
   */
  static async searchUser(params: SearchUserParams): Promise<UserPublicInfo> {
    return await invoke<UserPublicInfo>('search_user', {
      sessionToken: params.sessionToken,
      targetHash: params.targetHash,
    });
  }

  /**
   * Send a friend request to another user
   */
  static async sendFriendRequest(params: SendFriendRequestParams): Promise<string> {
    return await invoke<string>('send_friend_request', {
      sessionToken: params.sessionToken,
      targetHash: params.targetHash,
      targetPseudo: params.targetPseudo,
      message: params.message,
    });
  }

  /**
   * List all pending friend requests (incoming and outgoing)
   */
  static async listPendingRequests(params: ListParams): Promise<PendingRequest[]> {
    return await invoke<PendingRequest[]>('list_pending_requests', {
      sessionToken: params.sessionToken,
    });
  }

  /**
   * Accept an incoming friend request
   * @param params.pseudo - Optional custom name for the contact
   */
  static async acceptFriendRequest(params: FriendRequestResponseParams): Promise<string> {
    return await invoke<string>('accept_friend_request', {
      sessionToken: params.sessionToken,
      requesterHash: params.requesterHash,
      pseudo: params.pseudo,
    });
  }

  /**
   * Reject an incoming friend request
   */
  static async rejectFriendRequest(params: FriendRequestResponseParams): Promise<string> {
    return await invoke<string>('reject_friend_request', {
      sessionToken: params.sessionToken,
      requesterHash: params.requesterHash,
    });
  }

  /**
   * List all friends
   */
  static async listFriends(params: ListParams): Promise<FriendInfo[]> {
    return await invoke<FriendInfo[]>('list_friends', {
      sessionToken: params.sessionToken,
    });
  }

  /**
   * Remove a friend
   */
  static async removeFriend(params: RemoveFriendParams): Promise<string> {
    return await invoke<string>('remove_friend', {
      sessionToken: params.sessionToken,
      friendId: params.friendId,
    });
  }

  /**
   * Get the current user's public key (username hash) for sharing
   */
  static async getMyPublicKey(params: ListParams): Promise<string> {
    return await invoke<string>('get_my_public_key', {
      sessionToken: params.sessionToken,
    });
  }

  /**
   * Sync friend requests with the server (fetch incoming requests)
   */
  static async syncFriendRequests(params: ListParams): Promise<SyncResult> {
    return await invoke<SyncResult>('sync_friend_requests', {
      sessionToken: params.sessionToken,
    });
  }

  /**
   * Sync friend responses with the server (fetch responses to our outgoing requests)
   */
  static async syncFriendResponses(params: ListParams): Promise<SyncResult> {
    return await invoke<SyncResult>('sync_friend_responses', {
      sessionToken: params.sessionToken,
    });
  }

  static async blockFriend(params: RemoveFriendParams): Promise<void> {
    return await invoke('block_friend', {
      sessionToken: params.sessionToken,
      friendId: params.friendId,
    });
  }

  static async unblockFriend(params: RemoveFriendParams): Promise<void> {
    return await invoke('unblock_friend', {
      sessionToken: params.sessionToken,
      friendId: params.friendId,
    });
  }

  static async listBlockedFriends(params: ListParams): Promise<FriendInfo[]> {
    return await invoke<FriendInfo[]>('list_blocked_friends', {
      sessionToken: params.sessionToken,
    });
  }

  static async setMyAvatar(params: ListParams & { avatarB64: string }): Promise<void> {
    return await invoke('set_my_avatar', {
      sessionToken: params.sessionToken,
      avatarB64: params.avatarB64,
    });
  }

  static async getMyAvatar(params: ListParams): Promise<string | null> {
    return await invoke<string | null>('get_my_avatar', {
      sessionToken: params.sessionToken,
    });
  }

  static async setFriendAvatar(params: RemoveFriendParams & { avatarB64: string }): Promise<void> {
    return await invoke('set_friend_avatar', {
      sessionToken: params.sessionToken,
      friendId: params.friendId,
      avatarB64: params.avatarB64,
    });
  }

  static async renameFriend(sessionToken: string, friendId: number, newPseudo: string): Promise<void> {
    await invoke('rename_friend', { sessionToken, friendId, newPseudo });
  }

  /**
   * Initialise (ou retrouve) la conversation "Mon espace" (self-messaging).
   * Retourne l'id de l'entrée friend correspondante.
   */
  static async initSelfSpace(params: ListParams): Promise<number> {
    return await invoke<number>('init_self_space', {
      sessionToken: params.sessionToken,
    });
  }

  /**
   * Synchronise les contacts acceptés côté "responder" (METHOD 27).
   * Couvre le cas : ils ont envoyé la demande, on a accepté - non couvert par syncFriendResponses.
   */
  static async syncAcceptedContacts(params: ListParams): Promise<SyncResult> {
    return await invoke<SyncResult>('sync_accepted_contacts', {
      sessionToken: params.sessionToken,
    });
  }
}

/**
 * Result of sync operation
 */
export interface SyncResult {
  new_incoming: number;
  new_accepted: number;
  errors: string[];
}
