// TODO: Zod validation schemas
//
// This file will contain validation schemas for forms and data.

import { z } from 'zod';
import { DB_LIMITS } from './constants';

export const usernameSchema = z.string()
  .min(DB_LIMITS.MIN_USERNAME_LENGTH, `Username must be at least ${DB_LIMITS.MIN_USERNAME_LENGTH} characters`)
  .max(DB_LIMITS.MAX_USERNAME_LENGTH, `Username must be at most ${DB_LIMITS.MAX_USERNAME_LENGTH} characters`)
  .regex(/^[a-zA-Z0-9_-]+$/, 'Username can only contain letters, numbers, underscores, and hyphens');

export const passwordSchema = z.string()
  .min(DB_LIMITS.MIN_PASSWORD_LENGTH, `Password must be at least ${DB_LIMITS.MIN_PASSWORD_LENGTH} characters`)
  .max(DB_LIMITS.MAX_PASSWORD_LENGTH, `Password must be at most ${DB_LIMITS.MAX_PASSWORD_LENGTH} characters`);

export const keySchema = z.string()
  .length(DB_LIMITS.KEY_LENGTH, `Key must be exactly ${DB_LIMITS.KEY_LENGTH} characters`);

export const loginSchema = z.object({
  username: usernameSchema,
  password: passwordSchema,
});

export const registerSchema = z.object({
  username: usernameSchema,
  password: passwordSchema,
  confirmPassword: passwordSchema,
  key: keySchema,
}).refine((data) => data.password === data.confirmPassword, {
  message: "Passwords don't match",
  path: ["confirmPassword"],
});

export const messageSchema = z.object({
  content: z.string().max(DB_LIMITS.MAX_MESSAGE_LENGTH, `Message must be at most ${DB_LIMITS.MAX_MESSAGE_LENGTH} characters`),
  friendId: z.string(),
});

// TODO: Add more validation schemas as needed
