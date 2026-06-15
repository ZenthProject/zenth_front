import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/contexts/AuthContext";
import { useTheme } from "@/lib/theme";
import { SettingsData, defaultSettings } from "./types";
import i18n from "@/lib/i18n";

const SESSION_PW_KEY = 'zenth_session_pw';

export function useSettings() {
  const { sessionToken } = useAuth();
  const { theme: currentTheme, setTheme } = useTheme();

  // Initialise le thème depuis ThemeProvider (localStorage) - source de vérité
  const [settings, setSettings] = useState<SettingsData>(() => ({
    ...defaultSettings,
    theme: currentTheme === "system" ? "auto" : currentTheme,
    // Lire persist_session depuis localStorage immédiatement pour éviter que l'effet le réinitialise
    persist_session: localStorage.getItem("zenth_persist_session") === "true",
    // Lire la langue depuis localStorage pour éviter le reset à "en" au démarrage
    language: localStorage.getItem("zenth_language") ?? defaultSettings.language,
  }));
  const [isLoading, setIsLoading] = useState(false);
  const [saved, setSaved] = useState(false);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const savedTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const loadSettings = useCallback(async () => {
    if (!sessionToken) return;
    setIsLoading(true);
    try {
      const json = await invoke<string>("load_settings", { sessionToken });
      if (json && json !== "{}") {
        const parsed = JSON.parse(json);
        // Ne pas écraser le thème depuis la DB - ThemeProvider (localStorage) est la source de vérité
        const { theme: _ignored, ...rest } = parsed;
        setSettings(prev => ({ ...prev, ...rest }));
      }
    } catch (error) {
      console.error("Erreur chargement parametres:", error);
    } finally {
      setIsLoading(false);
    }
  }, [sessionToken]);

  useEffect(() => { loadSettings(); }, [loadSettings]);

  // Apparence
  // Thème → ThemeProvider ('auto' → 'system')
  useEffect(() => {
    const t = settings.theme;
    setTheme(t === "auto" ? "system" : (t as "dark" | "light"));
  }, [settings.theme, setTheme]);

  // Taille de police
  useEffect(() => {
    const html = document.documentElement;
    html.classList.remove("font-size-small", "font-size-medium", "font-size-large");
    if (settings.font_size !== "medium") {
      html.classList.add(`font-size-${settings.font_size}`);
    }
  }, [settings.font_size]);

  // Mode compact
  useEffect(() => {
    document.documentElement.dataset.compact = settings.compact_mode ? "true" : "false";
  }, [settings.compact_mode]);

  // Style des bulles → localStorage (lu par Chat)
  useEffect(() => {
    localStorage.setItem("zenth_bubble_style", settings.message_bubble_style);
    document.documentElement.style.setProperty("--bubble-style", settings.message_bubble_style);
  }, [settings.message_bubble_style]);

  // Langue → i18next + localStorage
  useEffect(() => {
    i18n.changeLanguage(settings.language);
    localStorage.setItem("zenth_language", settings.language);
  }, [settings.language]);

  // Notifications → localStorage (lu par notificationService)
  useEffect(() => {
    localStorage.setItem("zenth_notifications_enabled", settings.notifications_enabled ? "true" : "false");
    localStorage.setItem("zenth_notification_sound", settings.notification_sound ? "true" : "false");
  }, [settings.notifications_enabled, settings.notification_sound]);

  // Sécurité → localStorage (lu par useAutoLock et Login)
  useEffect(() => {
    localStorage.setItem("zenth_auto_lock_enabled", settings.auto_lock_enabled ? "true" : "false");
    localStorage.setItem("zenth_auto_lock_timeout", String(settings.auto_lock_timeout));
  }, [settings.auto_lock_enabled, settings.auto_lock_timeout]);

  useEffect(() => {
    if (!sessionToken) return;
    invoke("configure_wipe", {
      sessionToken,
      enabled: settings.wipe_after_failed_attempts,
      maxAttempts: settings.max_failed_attempts,
    }).catch(console.error);
  }, [settings.wipe_after_failed_attempts, settings.max_failed_attempts, sessionToken]);

  // Session persistante → localStorage (lu par AuthContext)
  const { username } = useAuth();
  useEffect(() => {
    const AUTH_KEY = 'zenth_auth';
    localStorage.setItem("zenth_persist_session", settings.persist_session ? "true" : "false");
    if (settings.persist_session) {
      // Stocke le credential si le mdp est disponible en session (mis par login())
      const pw = sessionStorage.getItem(SESSION_PW_KEY);
      if (pw && username) {
        void invoke('store_credential', { username, password: pw });
      }
    } else {
      // Supprime la session persistante et les credentials stockés
      localStorage.removeItem(AUTH_KEY); // migration : nettoie l'ancienne clé si présente
      void invoke('delete_credential');
    }
  }, [settings.persist_session]);

  // Identite → localStorage (lu par Friends)
  useEffect(() => {
    localStorage.setItem("zenth_auto_accept_friend_requests", settings.auto_accept_friend_requests ? "true" : "false");
  }, [settings.auto_accept_friend_requests]);

  const flashSaved = useCallback(() => {
    setSaved(true);
    if (savedTimer.current) clearTimeout(savedTimer.current);
    savedTimer.current = setTimeout(() => setSaved(false), 2000);
  }, []);

  // Opérations
  const saveSetting = useCallback(async (key: string, value: string | boolean) => {
    if (!sessionToken) return;
    try {
      await invoke("save_setting", {
        sessionToken,
        key,
        value: typeof value === "boolean" ? (value ? "true" : "false") : value,
      });
      flashSaved();
    } catch (error) {
      console.error("Erreur sauvegarde:", error);
    }
  }, [sessionToken, flashSaved]);

  const handleSwitchChange = useCallback((key: keyof SettingsData, value: boolean) => {
    setSettings(prev => ({ ...prev, [key]: value }));
    saveSetting(key, value);
  }, [saveSetting]);

  const handleSelectChange = useCallback((key: keyof SettingsData, value: string) => {
    setSettings(prev => ({ ...prev, [key]: value }));
    saveSetting(key, value);
  }, [saveSetting]);

  const handleInputChange = useCallback((key: keyof SettingsData, value: string) => {
    setSettings(prev => ({ ...prev, [key]: value }));
    if (saveTimer.current) clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(() => saveSetting(key, value), 600);
  }, [saveSetting]);

  return {
    settings,
    isLoading,
    saved,
    handleSwitchChange,
    handleSelectChange,
    handleInputChange,
  };
}
