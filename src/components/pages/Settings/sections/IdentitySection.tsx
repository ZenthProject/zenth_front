import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Users } from "lucide-react";
import { useTranslation } from "react-i18next";
import { SettingsCard } from "../SettingsCard";
import { SettingsData, SettingsHandlers } from "../types";

interface Props {
  settings: SettingsData;
  handlers: SettingsHandlers;
}

export function IdentitySection({ settings, handlers }: Props) {
  const { t } = useTranslation();
  return (
    <SettingsCard
      icon={Users}
      title={t("settings.identity.title")}
      description={t("settings.identity.subtitle")}
    >
      <div className="flex items-center justify-between">
        <div className="space-y-0.5">
          <Label className="text-foreground">{t("settings.identity.auto_accept_requests")}</Label>
          <p className="text-sm text-muted-foreground">{t("settings.identity.auto_accept_requests_sub")}</p>
        </div>
        <Switch
          checked={settings.auto_accept_friend_requests}
          onCheckedChange={(v) => handlers.handleSwitchChange("auto_accept_friend_requests", v)}
        />
      </div>
    </SettingsCard>
  );
}
