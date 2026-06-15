import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Palette } from "lucide-react";
import { useTheme } from "@/lib/theme";
import { useTranslation } from "react-i18next";
import { SettingsCard } from "../SettingsCard";
import { SettingsData, SettingsHandlers } from "../types";

interface Props {
  settings: SettingsData;
  handlers: SettingsHandlers;
}

export function AppearanceSection({ settings, handlers }: Props) {
  const { theme, setTheme } = useTheme();
  const { t } = useTranslation();
  const themeValue = theme === "system" ? "auto" : theme;

  const handleThemeChange = (v: string) => {
    setTheme(v === "auto" ? "system" : (v as "dark" | "light"));
    handlers.handleSelectChange("theme", v);
  };

  return (
    <SettingsCard icon={Palette} title={t("settings.appearance.title")}>
      <div className="grid grid-cols-2 gap-4">
        <div className="space-y-2">
          <Label>{t("settings.appearance.theme")}</Label>
          <Select value={themeValue} onValueChange={handleThemeChange}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="dark">{t("settings.appearance.theme_dark")}</SelectItem>
              <SelectItem value="light">{t("settings.appearance.theme_light")}</SelectItem>
              <SelectItem value="auto">{t("settings.appearance.theme_auto")}</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-2">
          <Label>{t("settings.appearance.language")}</Label>
          <Select value={settings.language} onValueChange={(v) => handlers.handleSelectChange("language", v)}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="fr">🇫🇷 Français</SelectItem>
              <SelectItem value="en">🇬🇧 English</SelectItem>
              <SelectItem value="de">🇩🇪 Deutsch</SelectItem>
              <SelectItem value="es">🇪🇸 Español</SelectItem>
              <SelectItem value="pt">🇵🇹 Português</SelectItem>
              <SelectItem value="ru">🇷🇺 Русский</SelectItem>
              <SelectItem value="zh">🇨🇳 中文</SelectItem>
              <SelectItem value="ja">🇯🇵 日本語</SelectItem>
              <SelectItem value="hi">🇮🇳 हिन्दी</SelectItem>
              <SelectItem value="it">🇮🇹 Italiano</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-2">
          <Label>{t("settings.appearance.font_size")}</Label>
          <Select value={settings.font_size} onValueChange={(v) => handlers.handleSelectChange("font_size", v)}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="small">{t("settings.appearance.font_small")}</SelectItem>
              <SelectItem value="medium">{t("settings.appearance.font_medium")}</SelectItem>
              <SelectItem value="large">{t("settings.appearance.font_large")}</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-2">
          <Label>{t("settings.appearance.bubble_style")}</Label>
          <Select value={settings.message_bubble_style} onValueChange={(v) => handlers.handleSelectChange("message_bubble_style", v)}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="rounded">{t("settings.appearance.bubble_rounded")}</SelectItem>
              <SelectItem value="square">{t("settings.appearance.bubble_square")}</SelectItem>
              <SelectItem value="minimal">{t("settings.appearance.bubble_minimal")}</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <div className="flex items-center justify-between">
        <Label>{t("settings.appearance.compact_mode")}</Label>
        <Switch
          checked={settings.compact_mode}
          onCheckedChange={(v) => handlers.handleSwitchChange("compact_mode", v)}
        />
      </div>
    </SettingsCard>
  );
}
