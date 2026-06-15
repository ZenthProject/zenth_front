// Zenth cryptographic key types based on protobuf DTOs
// These types mirror the structures defined in zenth_dto/proto/keys.proto

/**
 * Cryptographic signature algorithms
 */
export enum SignatureType {
  SIGNATURE_TYPE_UNKNOWN = 0,
  ED25519 = 1,
  DILITHIUM = 2,
}

/**
 * Key Encapsulation Mechanism types
 */
export enum KEMKeyType {
  KEM_KEY_TYPE_UNKNOWN = 0,
  X25519 = 1,
  KYBER512 = 2,
  KYBER768 = 3,
  KYBER1024 = 4,
}

/**
 * User identity key with post-quantum signature
 */
export interface IdentityKey {
  publicKey: Uint8Array;
  privateKey: Uint8Array;
  signatureType: SignatureType;
}

/**
 * Ephemeral key for key exchange
 */
export interface EphemeralKey {
  publicKey: Uint8Array;
  privateKey: Uint8Array;
  kemType: KEMKeyType;
}

/**
 * One-time pre-key
 */
export interface OneTimePreKey {
  id: number;
  publicKey: Uint8Array;
  privateKey: Uint8Array;
}
