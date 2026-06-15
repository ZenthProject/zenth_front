export interface SettingsData {
  // Apparence
  theme: string;
  language: string;
  font_size: string;
  compact_mode: boolean;
  message_bubble_style: string;

  // Securite
  auto_lock_enabled: boolean;
  auto_lock_timeout: string;
  wipe_after_failed_attempts: boolean;
  max_failed_attempts: string;

  // Session
  persist_session: boolean;

  // Identite et contacts
  auto_accept_friend_requests: boolean;

  // Notifications
  notifications_enabled: boolean;
  notification_sound: boolean;
}

export const defaultSettings: SettingsData = {
  // Apparence
  theme: "dark",
  language: "en",
  font_size: "medium",
  compact_mode: false,
  message_bubble_style: "rounded",

  // Securite
  auto_lock_enabled: true,
  auto_lock_timeout: "5",
  wipe_after_failed_attempts: false,
  max_failed_attempts: "10",

  // Session
  persist_session: false,

  // Identite et contacts
  auto_accept_friend_requests: false,

  // Notifications
  notifications_enabled: true,
  notification_sound: true,
};

export interface SettingsHandlers {
  handleSwitchChange: (key: keyof SettingsData, value: boolean) => void;
  handleSelectChange: (key: keyof SettingsData, value: string) => void;
  handleInputChange: (key: keyof SettingsData, value: string) => void;
}
