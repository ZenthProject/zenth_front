import { useState } from "react";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DialogClose,
} from "@/components/ui/dialog";
import { Lock, Trash2, AlertTriangle } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";
import { useAuth } from "@/contexts/AuthContext";
import { useTranslation } from "react-i18next";
import { SettingsCard } from "../SettingsCard";
import { SettingsData, SettingsHandlers } from "../types";

interface Props {
  settings: SettingsData;
  handlers: SettingsHandlers;
}

export function SecuritySection({ settings, handlers }: Props) {
  const { sessionToken, logout } = useAuth();
  const navigate = useNavigate();
  const { t } = useTranslation();
  const [isDeleting, setIsDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const handleDeleteAccount = async () => {
    if (!sessionToken) return;
    setIsDeleting(true);
    setDeleteError(null);
    try {
      await invoke("delete_account", { sessionToken });
      await logout();
      navigate("/login");
    } catch (e) {
      setDeleteError(String(e));
      setIsDeleting(false);
    }
  };

  return (
    <SettingsCard icon={Lock} title={t("settings.security.title")}>
      <div className="flex items-center justify-between">
        <div className="space-y-0.5">
          <Label className="text-foreground">{t("settings.security.persist_session")}</Label>
          <p className="text-sm text-muted-foreground">{t("settings.security.persist_session_sub")}</p>
        </div>
        <Switch
          checked={settings.persist_session}
          onCheckedChange={(v) => handlers.handleSwitchChange("persist_session", v)}
        />
      </div>

      {settings.persist_session && (
        <div className="flex items-start gap-2 rounded-md border border-yellow-500/40 bg-yellow-500/10 px-3 py-2 text-sm text-yellow-600 dark:text-yellow-400">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <span>{t("settings.security.persist_session_warning")}</span>
        </div>
      )}

      <Separator />

      <div className="flex items-center justify-between">
        <div className="space-y-0.5">
          <Label className="text-foreground">{t("settings.security.auto_lock")}</Label>
          <p className="text-sm text-muted-foreground">{t("settings.security.auto_lock_sub")}</p>
        </div>
        <Switch
          checked={settings.auto_lock_enabled}
          onCheckedChange={(v) => handlers.handleSwitchChange("auto_lock_enabled", v)}
        />
      </div>

      {settings.auto_lock_enabled && (
        <div className="pl-6 space-y-2">
          <Label className="text-foreground">{t("settings.security.auto_lock_delay")}</Label>
          <Select value={settings.auto_lock_timeout} onValueChange={(v) => handlers.handleSelectChange("auto_lock_timeout", v)}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="1">{t("settings.security.delay_1")}</SelectItem>
              <SelectItem value="5">{t("settings.security.delay_5")}</SelectItem>
              <SelectItem value="15">{t("settings.security.delay_15")}</SelectItem>
              <SelectItem value="30">{t("settings.security.delay_30")}</SelectItem>
            </SelectContent>
          </Select>
        </div>
      )}

      <Separator />

      <div className="flex items-center justify-between">
        <div className="space-y-0.5">
          <Label className="text-foreground">{t("settings.security.wipe_on_fail")}</Label>
          <p className="text-sm text-red-400">{t("settings.security.wipe_on_fail_sub")}</p>
        </div>
        <Switch
          checked={settings.wipe_after_failed_attempts}
          onCheckedChange={(v) => handlers.handleSwitchChange("wipe_after_failed_attempts", v)}
        />
      </div>

      {settings.wipe_after_failed_attempts && (
        <div className="pl-6 space-y-2">
          <Label className="text-foreground">{t("settings.security.wipe_attempts")}</Label>
          <Input
            type="number"
            value={settings.max_failed_attempts}
            onChange={(e) => handlers.handleInputChange("max_failed_attempts", e.target.value)}
            className="w-32"
          />
        </div>
      )}

      <Separator />

      <div className="flex items-center justify-between">
        <div className="space-y-0.5">
          <Label className="text-foreground">{t("settings.security.delete_account")}</Label>
          <p className="text-sm text-red-400">{t("settings.security.delete_account_sub")}</p>
        </div>
        <Dialog>
          <DialogTrigger asChild>
            <Button variant="destructive" size="sm" className="flex items-center gap-2">
              <Trash2 className="h-4 w-4" />
              {t("common.delete")}
            </Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle className="text-foreground">{t("settings.security.delete_dialog_title")}</DialogTitle>
              <DialogDescription className="text-muted-foreground">
                {t("settings.security.delete_dialog_body")}
              </DialogDescription>
            </DialogHeader>
            {deleteError && (
              <p className="text-sm text-red-400 px-1">{deleteError}</p>
            )}
            <DialogFooter className="gap-2">
              <DialogClose asChild>
                <Button variant="outline">{t("settings.security.delete_cancel")}</Button>
              </DialogClose>
              <Button
                onClick={handleDeleteAccount}
                disabled={isDeleting}
                className="bg-red-600 hover:bg-red-700 text-white"
              >
                {isDeleting ? t("settings.security.delete_loading") : t("settings.security.delete_confirm")}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </div>
    </SettingsCard>
  );
}
