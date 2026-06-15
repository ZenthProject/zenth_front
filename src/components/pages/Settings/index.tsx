import { Settings as SettingsIcon, Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useSettings } from "./useSettings";
import {
  AppearanceSection,
  SecuritySection,
  IdentitySection,
  NotificationsSection,
  SynchronizationSection,
  RecoverySection,
} from "./sections";

export default function Settings() {
  const { t } = useTranslation();
  const {
    settings,
    isLoading,
    saved,
    handleSwitchChange,
    handleSelectChange,
    handleInputChange,
  } = useSettings();

  const handlers = {
    handleSwitchChange,
    handleSelectChange,
    handleInputChange,
  };

  if (isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background">
        <Loader2 className="h-8 w-8 animate-spin text-primary" />
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto bg-background p-6">
      <div className="max-w-5xl mx-auto space-y-6">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <SettingsIcon className="h-8 w-8 text-accent-secondary" />
            <h1 className="text-3xl font-bold text-foreground">{t("settings.title")}</h1>
          </div>
          <span className={`text-xs text-muted-foreground transition-opacity duration-500 ${saved ? "opacity-100" : "opacity-0"}`}>
            {t("settings.saved")}
          </span>
        </div>

        <AppearanceSection settings={settings} handlers={handlers} />
        <NotificationsSection settings={settings} handlers={handlers} />
        <SecuritySection settings={settings} handlers={handlers} />
        <SynchronizationSection />
        <RecoverySection />
        <IdentitySection settings={settings} handlers={handlers} />
      </div>
    </div>
  );
}
