// Zenth session types based on protobuf DTOs
// These types mirror the structures defined in zenth_dto/proto/session.proto

import { KEMKeyType, IdentityKey } from "./keys";

/**
 * Individual message encryption keys
 */
export interface MessageKey {
  index: number;
  cipherKey: Uint8Array;
  macKey: Uint8Array;
  iv: Uint8Array;
}

/**
 * Ratchet chain state for message encryption
 */
export interface Chain {
  senderRatchetKey: Uint8Array;
  chainKeyIndex: number;
  chainKey: Uint8Array;
  messageKeys: MessageKey[];
}

/**
 * Post-quantum ratchet using Kyber KEM
 */
export interface PQRatchetState {
  kemType: KEMKeyType;
  currentPqKey: Uint8Array;
  pqRatchetIndex: number;
  pendingPqKey: Uint8Array;
}

/**
 * Complete session state with hybrid ratcheting
 */
export interface SessionState {
  sessionVersion: number;
  localIdentity: IdentityKey;
  remoteIdentity: IdentityKey;
  rootKey: Uint8Array;
  senderChain: Chain;
  receiverChains: Chain[];
  pqRatchet: PQRatchetState;
  previousCounter: number;
  remoteRegistrationId: number;
  localRegistrationId: number;
  needsRefresh: boolean;
  lastRefreshTimestamp: bigint;
}

/**
 * Peer-to-peer session with user identifiers
 */
export interface Session {
  localUserHashId: Uint8Array;
  remoteUserHashId: Uint8Array;
  sessionId: Uint8Array;
  state: SessionState;
  createdTimestamp: bigint;
  lastUsedTimestamp: bigint;
  isActive: boolean;
}

/**
 * Local storage for all active sessions
 */
export interface SessionStore {
  sessions: Session[];
  ownerHashId: Uint8Array;
}

/**
 * Ephemeral pre-key for initial key exchange
 */
export interface PreKeyRecord {
  id: number;
  publicKey: Uint8Array;
  privateKey: Uint8Array;
}

/**
 * Signed pre-key with Dilithium signature
 */
export interface SignedPreKeyRecord {
  id: number;
  publicKey: Uint8Array;
  privateKey: Uint8Array;
  signature: Uint8Array;
  timestamp: bigint;
}

/**
 * Post-quantum KEM pre-key
 */
export interface PQPreKeyRecord {
  id: number;
  keyType: KEMKeyType;
  publicKey: Uint8Array;
  secretKey: Uint8Array;
  timestamp: bigint;
}

/**
 * Local storage for all pre-key types
 */
export interface PreKeyStore {
  preKeys: PreKeyRecord[];
  signedPreKeys: SignedPreKeyRecord[];
  pqPreKeys: PQPreKeyRecord[];
  nextPreKeyId: number;
  nextSignedPreKeyId: number;
  nextPqPreKeyId: number;
}
