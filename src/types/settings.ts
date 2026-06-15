// TODO: Settings types
//
// This file will define TypeScript types for application settings.
// Should mirror the Rust AppSettings structure.

export interface AppearanceSettings {
  theme: 'light' | 'dark' | 'system';
  language: string;
  fontSize: number;
  compactMode: boolean;
}

export interface CryptographySettings {
  keyRotationIntervalDays: number;
  signatureAlgorithm: 'dilithium5';
  kemAlgorithm: 'kyber1024';
  hashAlgorithm: 'sha3-512' | 'blake3';
}

export interface NetworkSettings {
  darknet: 'tor' | 'i2p' | 'lokinet';
  torCircuitHops: number;
  connectionTimeout: number;
  maxRetries: number;
}

export interface SecuritySettings {
  autoLockMinutes: number;
  wipeAfterFailedAttempts: number;
  requirePinForSensitiveActions: boolean;
}

// TODO: Add remaining settings categories (forward secrecy, ephemeral, metadata, etc.)

export interface AppSettings {
  appearance: AppearanceSettings;
  cryptography: CryptographySettings;
  network: NetworkSettings;
  security: SecuritySettings;
  // ... more categories
}
