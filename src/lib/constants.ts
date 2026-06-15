// Application constants

export const API_ROUTES = {
  REGISTER: '/api/register',
  LOGIN: '/api/login',
  CHAT: '/api/chat',
  FRIENDS: '/api/friends',
  TRACEROUTE: '/api/traceroute',
} as const;

export const DB_LIMITS = {
  MAX_FILE_SIZE: 50 * 1024 * 1024, // 50MB
  MAX_MESSAGE_LENGTH: 10000,
  MAX_USERNAME_LENGTH: 32,
  MIN_USERNAME_LENGTH: 2,
  MAX_PASSWORD_LENGTH: 128,
  MIN_PASSWORD_LENGTH: 20,
  KEY_LENGTH: 20000,
} as const;

export const CRYPTO_PARAMS = {
  ARGON2_TIME_COST: 3,
  ARGON2_MEMORY_COST: 65536,
  ARGON2_PARALLELISM: 4,
  DILITHIUM_VARIANT: 'dilithium5',
  KYBER_VARIANT: 'kyber1024',
  HASH_ALGORITHM: 'sha3-512',
} as const;

export const UI = {
  TOAST_DURATION: 3000,
  SIDEBAR_WIDTH: 240,
  SIDEBAR_WIDTH_COLLAPSED: 60,
} as const;
