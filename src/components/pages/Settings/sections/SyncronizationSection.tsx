import { Label } from "@/components/ui/label";
import { Users } from "lucide-react";
import { useTranslation } from "react-i18next";
import { SettingsCard } from "../SettingsCard";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
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
import { RotateCcw } from "lucide-react";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/contexts/AuthContext";
import { QRCodeSVG } from "qrcode.react";
import { QRScanner } from "@/components/modules/QRScanner";
import { useNavigate } from "react-router-dom";

// Types
interface PairedDevice {
  pubkey_hex: string;
  added_at: number;
}

type Role = "new_device" | "trusted_device";

type PairingStep =
  | "choose_role"
  | "confirm_password"
  | "show_qr"        // nouvel appareil : affiche son petit QR (pid)
  | "scan_new"       // appareil de confiance : scanne le QR du nouvel appareil
  | "show_return_qr" // appareil de confiance : affiche son petit QR retour
  | "scan_return"    // nouvel appareil : scanne le QR retour
  | "verifying"
  | "fetching_key"
  | "success"
  | "error";

interface PairingQrResult {
  return_qr: string;
  dilithium_pubkey_pc: string;
  kyber_pubkey_pc: string;
}

// Composant
export function SynchronizationSection() {
  const { sessionToken, logout } = useAuth();
  const { t } = useTranslation();
  const navigate = useNavigate();

  const [role, setRole] = useState<Role>("new_device");
  const [step, setStep] = useState<PairingStep>("choose_role");
  const [error, setError] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [isBusy, setIsBusy] = useState(false);
  const [pairedCount, setPairedCount] = useState<number | null>(null);
  const [pairedDevices, setPairedDevices] = useState<PairedDevice[]>([]);
  const [confirmRevokeId, setConfirmRevokeId] = useState<string | null>(null);
  const [revoking, setRevoking] = useState<string | null>(null);

  const loadPairedDevices = () => {
    if (!sessionToken) return;
    invoke<PairedDevice[]>("list_paired_devices", { sessionToken })
      .then((devices) => {
        setPairedDevices(devices);
        setPairedCount(devices.length);
      })
      .catch(() => {});
  };

  useEffect(() => {
    loadPairedDevices();
  }, [sessionToken]);

  const handleRevoke = async (pubkeyHex: string) => {
    setRevoking(pubkeyHex);
    setConfirmRevokeId(null);
    try {
      await invoke("revoke_paired_device", { sessionToken, pubkeyHex });
      loadPairedDevices();
    } finally {
      setRevoking(null);
    }
  };

  // QR minuscule affiché par cet appareil (nouvel ou retour)
  const [myQrPayload, setMyQrPayload] = useState<string | null>(null);

  // QR retour (appareil de confiance)
  const [returnQrPayload, setReturnQrPayload] = useState<string | null>(null);

  // pubkey Dilithium de l'appareil de confiance (pour fetch_sync_key)
  const [trustedDilithiumPubkey, setTrustedDilithiumPubkey] = useState<string | null>(null);
  // username_hash_hex de l'appareil de confiance (pour nettoyage du compte temporaire)
  const [trustedUsernameHashHex, setTrustedUsernameHashHex] = useState<string>("");
  // username en clair de l'appareil de confiance (affiché au succès pour guider la reconnexion)
  const [trustedUsername, setTrustedUsername] = useState<string>("");

  const handleLogoutAndReconnect = async () => {
    if (trustedUsername) {
      localStorage.setItem('zenth_remember_username', trustedUsername);
    }
    await logout();
    navigate("/login");
  };

  const reset = () => {
    setStep("choose_role");
    setError(null);
    setPassword("");
    setIsBusy(false);
    setMyQrPayload(null);
    setReturnQrPayload(null);
    setTrustedDilithiumPubkey(null);
    setTrustedUsernameHashHex("");
    setTrustedUsername("");
    setConfirmRevokeId(null);
  };

  // Rôle : NOUVEL APPAREIL
  const handleNewDeviceConfirmPassword = async () => {
    if (!password.trim()) return;
    setIsBusy(true);
    setError(null);
    try {
      // Publie les clés sur le DHT, reçoit le QR minuscule (pid + h)
      const qrJson = await invoke<string>("publish_pairing_keys", {
        sessionToken,
        password,
      });
      setMyQrPayload(qrJson);
      setStep("show_qr");
    } catch (e) {
      setError(String(e));
    } finally {
      setIsBusy(false);
    }
  };

  const handleScanReturnQr = async (qrPayload: string) => {
    setError(null);
    setStep("verifying");
    try {
      const result = await invoke<{ pubkey: string; username_hash_hex: string; trusted_username: string }>("verify_pairing_qr", {
        sessionToken,
        qrJson: qrPayload,
      });
      const pubkeyTel = result.pubkey;
      const hashHex = result.username_hash_hex ?? "";
      const uname = result.trusted_username ?? "";

      setTrustedDilithiumPubkey(pubkeyTel);
      setTrustedUsernameHashHex(hashHex);
      setTrustedUsername(uname);
      setStep("fetching_key");

      const syncResult = await invoke<{ effective_username: string }>("fetch_sync_key", {
        sessionToken,
        dilithiumPubkeyTelBase64: pubkeyTel,
        trustedUsernameHashHex: hashHex,
        trustedUsername: uname,
      });
      const effectiveUsername = syncResult?.effective_username || uname;
      setTrustedUsername(effectiveUsername);

      // Récupère immédiatement les contacts poussés par l'appareil de confiance.
      invoke("relay_pull_messages", { sessionToken }).catch(() => {});

      setStep("success");
    } catch (e) {
      const msg = String(e);
      setError(msg.includes("sync_account_conflict")
        ? t("settings.synchronization.error_account_conflict")
        : msg);
      setStep("error");
    }
  };

  const handleRetryFetchKey = async () => {
    if (!trustedDilithiumPubkey) return;
    setIsBusy(true);
    setError(null);
    setStep("fetching_key");
    try {
      const retryResult = await invoke<{ effective_username: string }>("fetch_sync_key", {
        sessionToken,
        dilithiumPubkeyTelBase64: trustedDilithiumPubkey,
        trustedUsernameHashHex,
        trustedUsername,
      });
      if (retryResult?.effective_username) {
        setTrustedUsername(retryResult.effective_username);
      }
      invoke("relay_pull_messages", { sessionToken }).catch(() => {});
      setStep("success");
    } catch (e) {
      const msg = String(e);
      setError(msg.includes("sync_account_conflict")
        ? t("settings.synchronization.error_account_conflict")
        : msg);
      setStep("error");
    } finally {
      setIsBusy(false);
    }
  };

  // Rôle : APPAREIL DE CONFIANCE
  const handleScanNewDevice = async (qrPayload: string) => {
    try {
      const parsed = JSON.parse(qrPayload);
      if (!parsed.pid || !parsed.h || parsed.v !== "1") {
        setError(t("settings.synchronization.error_invalid_qr"));
        return;
      }
    } catch {
      setError(t("settings.synchronization.error_invalid_qr_format"));
      return;
    }
    setMyQrPayload(qrPayload);
    setStep("confirm_password");
  };

  const handleTrustedDeviceConfirmPassword = async () => {
    if (!password.trim() || !myQrPayload) return;
    setIsBusy(true);
    setError(null);
    try {
      const result = await invoke<PairingQrResult>("generate_pairing_qr", {
        sessionToken,
        password,
        scannedQrJson: myQrPayload,
      });

      // Envoie la Sync Key sur le DHT AVANT d'afficher le QR retour -
      // sinon device B appelle fetch_sync_key avant que le blob existe.
      await invoke("send_sync_key", {
        sessionToken,
        kyberPubkeyPcBase64:     result.kyber_pubkey_pc,
        dilithiumPubkeyPcBase64: result.dilithium_pubkey_pc,
      });

      // Pousse tous les contacts (avec pseudos) vers le nouvel appareil.
      // Best-effort : ne bloque pas l'affichage du QR retour si ça échoue.
      invoke("relay_push_all_contacts", { sessionToken }).catch(() => {});

      setReturnQrPayload(result.return_qr);
      setStep("show_return_qr");
    } catch (e) {
      setError(String(e));
    } finally {
      setIsBusy(false);
    }
  };

  // Rendu des étapes
  const renderStep = () => {
    switch (step) {

      case "choose_role":
        return (
          <div className="flex flex-col gap-3 py-4">
            <Button
              onClick={() => { setRole("new_device"); setStep("confirm_password"); }}
              className="w-full"
            >
              {t("settings.synchronization.role_new_device")}
            </Button>
            <Button
              variant="outline"
              onClick={() => { setRole("trusted_device"); setStep("scan_new"); }}
              className="w-full"
            >
              {t("settings.synchronization.role_trusted_device")}
            </Button>
          </div>
        );

      case "confirm_password":
        return (
          <div className="flex flex-col gap-4 py-4">
            <p className="text-sm text-muted-foreground">
              {t("settings.synchronization.password_required")}
            </p>
            <Input
              type="password"
              placeholder={t("settings.synchronization.password_placeholder")}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  role === "new_device"
                    ? handleNewDeviceConfirmPassword()
                    : handleTrustedDeviceConfirmPassword();
                }
              }}
              autoFocus
            />
            {error && <p className="text-sm text-red-400">{error}</p>}
            <Button
              onClick={role === "new_device"
                ? handleNewDeviceConfirmPassword
                : handleTrustedDeviceConfirmPassword}
              disabled={isBusy || !password.trim()}
              className="w-full"
            >
              {isBusy
                ? t("settings.synchronization.verifying")
                : t("settings.synchronization.confirm_password_btn")}
            </Button>
          </div>
        );

      // Nouvel appareil : affiche son petit QR (pid)
      case "show_qr":
        return (
          <div className="flex flex-col items-center gap-4 py-4">
            <p className="text-sm text-muted-foreground text-center">
              {t("settings.synchronization.scan_instruction_step1")}
            </p>
            {myQrPayload && (
              <>
                <div className="mx-auto w-full max-w-[220px] aspect-square bg-white rounded-lg p-2 flex items-center justify-center">
                  <QRCodeSVG value={myQrPayload} size={512} level="M" style={{ width: "100%", height: "auto", display: "block" }} />
                </div>
                <p className="text-xs text-muted-foreground text-center opacity-60">
                  {t("settings.synchronization.copy_qr_hint")}
                </p>
                <pre
                  className="text-xs bg-muted rounded p-2 w-full whitespace-pre-wrap break-all cursor-pointer select-all"
                  onClick={() => navigator.clipboard?.writeText(myQrPayload)}
                  title="Cliquer pour copier"
                >
                  {myQrPayload}
                </pre>
              </>
            )}
            <Button
              variant="outline"
              size="sm"
              onClick={() => setStep("scan_return")}
              className="w-full"
            >
              {t("settings.synchronization.scan_return_btn")}
            </Button>
          </div>
        );

      // Nouvel appareil : scanne le QR retour
      case "scan_return":
        return (
          <div className="flex flex-col items-center gap-4 py-4">
            <p className="text-sm text-muted-foreground text-center">
              {t("settings.synchronization.scan_return_instruction")}
            </p>
            <QRScanner
              onScan={handleScanReturnQr}
              onError={(err) => setError(err)}
            />
          </div>
        );

      // Appareil de confiance : scanne le QR du nouvel appareil
      case "scan_new":
        return (
          <div className="flex flex-col items-center gap-4 py-4">
            <p className="text-sm text-muted-foreground text-center">
              {t("settings.synchronization.scan_new_instruction")}
            </p>
            <QRScanner
              onScan={handleScanNewDevice}
              onError={(err) => setError(err)}
            />
            {error && <p className="text-sm text-red-400 text-center">{error}</p>}
          </div>
        );

      // Appareil de confiance : affiche le QR retour (sync key déjà envoyée)
      case "show_return_qr":
        return (
          <div className="flex flex-col items-center gap-4 py-4">
            <p className="text-sm text-muted-foreground text-center">
              {t("settings.synchronization.show_return_instruction")}
            </p>
            {returnQrPayload && (
              <>
                <div className="mx-auto w-full max-w-[220px] aspect-square bg-white rounded-lg p-2 flex items-center justify-center">
                  <QRCodeSVG value={returnQrPayload} size={512} level="M" style={{ width: "100%", height: "auto", display: "block" }} />
                </div>
                <pre
                  className="text-xs bg-muted rounded p-2 w-full whitespace-pre-wrap break-all cursor-pointer select-all"
                  onClick={() => navigator.clipboard?.writeText(returnQrPayload)}
                  title="Cliquer pour copier"
                >
                  {returnQrPayload}
                </pre>
              </>
            )}
            <p className="text-xs text-amber-400 text-center">
              {t("settings.synchronization.return_qr_expiry")}
            </p>
            <p className="text-xs text-green-400 text-center">
              {t("settings.synchronization.sync_key_sent")}
            </p>
          </div>
        );

      case "verifying":
        return (
          <div className="flex flex-col items-center gap-2 py-6">
            <p className="text-sm text-muted-foreground">
              {t("settings.synchronization.verifying")}
            </p>
          </div>
        );

      case "fetching_key":
        return (
          <div className="flex flex-col items-center gap-2 py-6">
            <p className="text-sm text-muted-foreground">
              {t("settings.synchronization.fetching_key")}
            </p>
          </div>
        );

      case "success":
        loadPairedDevices();
        return (
          <div className="flex flex-col items-center gap-4 py-6">
            <p className="text-sm text-green-400 text-center">
              {t("settings.synchronization.success")}
            </p>
            {role === "new_device" && (
              <>
                <p className="text-xs text-muted-foreground text-center">
                  {trustedUsername
                    ? t("settings.synchronization.success_reconnect_hint_named", { username: trustedUsername })
                    : t("settings.synchronization.success_reconnect_hint")}
                </p>
                <Button size="sm" onClick={handleLogoutAndReconnect}>
                  {t("settings.synchronization.reconnect_btn")}
                </Button>
              </>
            )}
          </div>
        );

      case "error":
        return (
          <p className="text-sm text-red-400 px-1 py-4">{error}</p>
        );
    }
  };

  return (
    <SettingsCard icon={Users} title={t("settings.synchronization.title")}>
      <div className="flex flex-col gap-4">
        <div className="flex items-center justify-between">
          <div className="space-y-0.5">
            <Label className="text-foreground">
              {t("settings.security.sync_accounts")}
            </Label>
            <p className="text-sm text-red-400">
              {t("settings.security.sync_accounts_sub")}
            </p>
            {pairedCount !== null && (
              <p className={`text-xs ${pairedCount > 0 ? "text-green-400" : "text-amber-400"}`}>
                {pairedCount > 0
                  ? t("settings.synchronization.paired_devices", { count: pairedCount })
                  : t("settings.synchronization.no_paired_devices")}
              </p>
            )}
          </div>

          <Dialog onOpenChange={(open) => { if (!open) reset(); }}>
            <DialogTrigger asChild>
              <Button
                variant="destructive"
                size="sm"
                className="flex items-center gap-2"
              >
                <RotateCcw className="h-4 w-4" />
                {t("common.sync")}
              </Button>
            </DialogTrigger>

            <DialogContent className="overflow-y-auto max-h-[90vh]">
              <DialogHeader>
                <DialogTitle className="text-foreground">
                  {t("settings.synchronization.dialog_title")}
                </DialogTitle>
                <DialogDescription className="text-muted-foreground">
                  {t("settings.synchronization.dialog_description")}
                </DialogDescription>
              </DialogHeader>

              {renderStep()}

              <DialogFooter className="gap-2">
                <DialogClose asChild>
                  <Button variant="outline">
                    {t("settings.synchronization.cancel")}
                  </Button>
                </DialogClose>
                {step === "error" && trustedDilithiumPubkey && (
                  <Button variant="outline" onClick={handleRetryFetchKey} disabled={isBusy}>
                    {t("settings.synchronization.retry_key")}
                  </Button>
                )}
                {step === "error" && (
                  <Button variant="outline" onClick={reset}>
                    {t("settings.synchronization.retry")}
                  </Button>
                )}
                {step === "show_return_qr" && (
                  <Button onClick={() => setStep("success")}>
                    {t("settings.synchronization.done_btn")}
                  </Button>
                )}
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </div>

        {pairedDevices.length > 0 && (
          <div className="space-y-2">
            <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
              {t("settings.synchronization.devices_list_title")}
            </p>
            {pairedDevices.map((device) => {
              const shortId = `${device.pubkey_hex.slice(0, 8)}...${device.pubkey_hex.slice(-8)}`;
              const dateStr = new Date(device.added_at * 1000).toLocaleDateString();
              const isConfirming = confirmRevokeId === device.pubkey_hex;
              const isRevoking = revoking === device.pubkey_hex;

              return (
                <div key={device.pubkey_hex} className="flex items-center justify-between rounded-md border border-border px-3 py-2 text-xs">
                  <div className="flex flex-col gap-0.5">
                    <span className="font-mono text-foreground">{shortId}</span>
                    <span className="text-muted-foreground">
                      {t("settings.synchronization.device_added_on", { date: dateStr })}
                    </span>
                  </div>
                  <div className="flex items-center gap-2 ml-2 shrink-0">
                    {isConfirming ? (
                      <>
                        <span className="text-muted-foreground">
                          {t("settings.synchronization.revoke_confirm_title")}
                        </span>
                        <Button
                          variant="destructive"
                          size="sm"
                          className="h-6 px-2 text-xs"
                          onClick={() => handleRevoke(device.pubkey_hex)}
                          disabled={isRevoking}
                        >
                          {t("settings.synchronization.revoke_confirm_btn")}
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-6 px-2 text-xs"
                          onClick={() => setConfirmRevokeId(null)}
                        >
                          {t("common.cancel")}
                        </Button>
                      </>
                    ) : (
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-6 px-2 text-xs text-red-400 hover:text-red-300 hover:bg-red-950/30"
                        onClick={() => setConfirmRevokeId(device.pubkey_hex)}
                        disabled={isRevoking}
                      >
                        {isRevoking
                          ? t("settings.synchronization.revoking")
                          : t("settings.synchronization.revoke_device")}
                      </Button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </SettingsCard>
  );
}
