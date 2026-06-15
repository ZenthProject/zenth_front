import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Bell } from "lucide-react";
import { useTranslation } from "react-i18next";
import { SettingsCard } from "../SettingsCard";
import { SettingsData, SettingsHandlers } from "../types";

interface Props {
  settings: SettingsData;
  handlers: SettingsHandlers;
}

export function NotificationsSection({ settings, handlers }: Props) {
  const { t } = useTranslation();
  return (
    <SettingsCard icon={Bell} title={t("settings.notifications.title")}>
      <div className="flex items-center justify-between">
        <Label className="text-foreground">{t("settings.notifications.enable")}</Label>
        <Switch
          checked={settings.notifications_enabled}
          onCheckedChange={(v) => handlers.handleSwitchChange("notifications_enabled", v)}
        />
      </div>

      {settings.notifications_enabled && (
        <div className="pl-6 flex items-center justify-between">
          <Label className="text-foreground">{t("settings.notifications.sound")}</Label>
          <Switch
            checked={settings.notification_sound}
            onCheckedChange={(v) => handlers.handleSwitchChange("notification_sound", v)}
          />
        </div>
      )}
    </SettingsCard>
  );
}
