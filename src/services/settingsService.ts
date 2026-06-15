import { invoke } from '@tauri-apps/api/core';

interface AuthParams {
  sessionToken: string;
}

export class SettingsService {
  static async loadSettings({ sessionToken }: AuthParams): Promise<string> {
    return await invoke('load_settings', { sessionToken });
  }

  static async saveSetting({ sessionToken }: AuthParams, key: string, value: string): Promise<void> {
    await invoke('save_setting', { sessionToken, key, value });
  }

  static async saveAllSettings({ sessionToken }: AuthParams, settings: string): Promise<void> {
    await invoke('save_all_settings', { sessionToken, settings });
  }

  static async getSetting({ sessionToken }: AuthParams, key: string): Promise<string> {
    return await invoke('get_setting', { sessionToken, key });
  }

  static async resetSettings({ sessionToken }: AuthParams): Promise<void> {
    await invoke('reset_settings', { sessionToken });
  }

  static async deleteSetting({ sessionToken }: AuthParams, key: string): Promise<void> {
    await invoke('delete_setting', { sessionToken, key });
  }
}
