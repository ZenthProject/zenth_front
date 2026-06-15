/**
 * Types for friend management
 */

/**
 * User public information returned from search
 */
export interface UserPublicInfo {
  username_hash: string;
  identity_key: string;  // hex encoded
  kyber_public_key: string;  // hex encoded
  x25519_public_key?: string;  // hex encoded, optional
}

/**
 * Friend information
 */
export interface FriendInfo {
  id: number;
  pseudo: string;
  username_hash: string;
  identity_key_public: string;  // hex encoded
  kyber_public_key?: string;  // hex encoded
  x25519_public_key?: string;  // hex encoded
  verified: boolean;
  blocked: boolean;
  created_at: number;
  avatar?: string;  // base64 encoded image BLOB
}

/**
 * Pending friend request
 */
export interface PendingRequest {
  id: number;
  direction: 'incoming' | 'outgoing';
  remote_username_hash: string;
  remote_pseudo?: string;
  remote_identity_key: string;  // hex encoded
  message?: string;
  created_at: number;
  expires_at?: number;
}

/**
 * Parameters for sending a friend request
 */
export interface SendFriendRequestParams {
  sessionToken: string;
  targetHash: string;
  targetPseudo?: string;
  message?: string;
}

/**
 * Parameters for accepting/rejecting a friend request
 */
export interface FriendRequestResponseParams {
  sessionToken: string;
  requesterHash: string;
  pseudo?: string;  // Custom name for the contact (optional)
}

/**
 * Parameters for listing friends or pending requests
 */
export interface ListParams {
  sessionToken: string;
}

/**
 * Parameters for searching a user
 */
export interface SearchUserParams {
  sessionToken: string;
  targetHash: string;
}

/**
 * Parameters for removing a friend
 */
export interface RemoveFriendParams {
  sessionToken: string;
  friendId: number;
}
