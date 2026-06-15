import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/contexts/AuthContext";
import { useTranslation } from "react-i18next";
import { SettingsCard } from "../SettingsCard";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Checkbox } from "@/components/ui/checkbox";
import { Textarea } from "@/components/ui/textarea";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import {
  ShieldCheck,
  ShieldAlert,
  KeyRound,
  Download,
  Upload,
  Copy,
  Check,
  AlertTriangle,
  Loader2,
  FileArchive,
} from "lucide-react";
import type { FriendInfo } from "@/types/friends";

// Types
type InitStep = "display" | "confirm" | "done";
type ExportStep = "select" | "passwords" | "exporting" | "done";
type ImportStep = "input" | "importing" | "done";

interface ImportResult {
  contacts_imported: number;
  messages_imported: number;
  has_recovery_key: boolean;
}

interface FriendSelection {
  profile: boolean;
  messages: boolean;
}

// Helpers
function WordGrid({ words }: { words: string[] }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(words.join(" "));
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-3 gap-2 rounded-lg border border-border bg-muted/30 p-4 font-mono text-sm">
        {words.map((word, i) => (
          <div key={i} className="flex items-center gap-1.5">
            <span className="w-5 text-right text-xs text-muted-foreground">{i + 1}.</span>
            <span className="font-medium text-foreground">{word}</span>
          </div>
        ))}
      </div>
      <Button variant="outline" size="sm" className="w-full gap-2" onClick={handleCopy}>
        {copied ? <Check className="h-4 w-4 text-green-500" /> : <Copy className="h-4 w-4" />}
        {copied ? "Copié !" : "Copier la phrase"}
      </Button>
    </div>
  );
}

// Init dialog
interface InitDialogProps {
  open: boolean;
  onClose: () => void;
  onDone: () => void;
  sessionToken: string;
}

function InitDialog({ open, onClose, onDone, sessionToken }: InitDialogProps) {
  const { t } = useTranslation();
  const [step, setStep] = useState<InitStep>("display");
  const [words, setWords] = useState<string[]>([]);
  const [pubkey, setPubkey] = useState("");
  const [verifyPositions, setVerifyPositions] = useState<number[]>([]);
  const [verifyInputs, setVerifyInputs] = useState<string[]>(["", "", ""]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Generate key when dialog opens
  useEffect(() => {
    if (!open) return;
    setStep("display");
    setError(null);
    setLoading(true);
    invoke<{ words: string[]; pubkey_hex: string }>("init_recovery_key", { sessionToken })
      .then((res) => {
        setWords(res.words);
        setPubkey(res.pubkey_hex);
        // Pick 3 random positions (0-indexed)
        const positions: number[] = [];
        while (positions.length < 3) {
          const p = Math.floor(Math.random() * 24);
          if (!positions.includes(p)) positions.push(p);
        }
        positions.sort((a, b) => a - b);
        setVerifyPositions(positions);
        setVerifyInputs(["", "", ""]);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [open]);

  const handleVerify = async () => {
    setError(null);
    const ok = verifyPositions.every(
      (pos, i) => verifyInputs[i].trim().toLowerCase() === words[pos]
    );
    if (!ok) {
      setError(t("settings.recovery.confirm_error"));
      return;
    }
    setStep("done");
  };

  const handleClose = () => {
    if (step === "done") onDone();
    onClose();
  };

  return (
    <Dialog open={open} onOpenChange={(v) => !v && handleClose()}>
      <DialogContent className="max-w-lg">
        {step === "display" && (
          <>
            <DialogHeader>
              <DialogTitle className="text-foreground">{t("settings.recovery.init_title")}</DialogTitle>
              <DialogDescription className="text-muted-foreground">
                {t("settings.recovery.init_subtitle")}
              </DialogDescription>
            </DialogHeader>

            {loading && (
              <div className="flex items-center justify-center py-8">
                <Loader2 className="h-6 w-6 animate-spin text-primary" />
              </div>
            )}

            {error && (
              <p className="text-sm text-red-400">{error}</p>
            )}

            {!loading && words.length > 0 && (
              <>
                <div className="flex items-start gap-2 rounded-md border border-yellow-500/40 bg-yellow-500/10 px-3 py-2 text-sm text-yellow-600 dark:text-yellow-400">
                  <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
                  <span>{t("settings.recovery.init_warning")}</span>
                </div>
                <WordGrid words={words} />
              </>
            )}

            <DialogFooter>
              <Button variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
              <Button
                disabled={loading || words.length === 0}
                onClick={() => setStep("confirm")}
              >
                {t("settings.recovery.init_next")}
              </Button>
            </DialogFooter>
          </>
        )}

        {step === "confirm" && (
          <>
            <DialogHeader>
              <DialogTitle className="text-foreground">{t("settings.recovery.confirm_title")}</DialogTitle>
              <DialogDescription className="text-muted-foreground">
                {t("settings.recovery.confirm_subtitle")}
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-3">
              {verifyPositions.map((pos, i) => (
                <div key={pos} className="flex items-center gap-3">
                  <Label className="w-20 shrink-0 text-right text-muted-foreground">
                    {t("settings.recovery.word_n", { n: pos + 1 })}
                  </Label>
                  <Input
                    value={verifyInputs[i]}
                    onChange={(e) => {
                      const next = [...verifyInputs];
                      next[i] = e.target.value;
                      setVerifyInputs(next);
                      setError(null);
                    }}
                    placeholder="..."
                    className="font-mono"
                    onKeyDown={(e) => e.key === "Enter" && handleVerify()}
                  />
                </div>
              ))}
            </div>

            {error && <p className="text-sm text-red-400">{error}</p>}

            <DialogFooter>
              <Button variant="outline" onClick={() => setStep("display")}>{t("common.back")}</Button>
              <Button onClick={handleVerify}>{t("settings.recovery.confirm_btn")}</Button>
            </DialogFooter>
          </>
        )}

        {step === "done" && (
          <>
            <DialogHeader>
              <DialogTitle className="text-foreground">{t("settings.recovery.init_done_title")}</DialogTitle>
              <DialogDescription className="text-muted-foreground">
                {t("settings.recovery.init_done_subtitle")}
              </DialogDescription>
            </DialogHeader>

            <div className="flex flex-col items-center gap-3 py-4">
              <ShieldCheck className="h-12 w-12 text-green-500" />
              {pubkey && (
                <p className="text-center font-mono text-xs text-muted-foreground break-all">
                  {t("settings.recovery.pubkey_label")}: {pubkey.slice(0, 16)}…
                </p>
              )}
            </div>

            <DialogFooter>
              <Button onClick={handleClose}>{t("common.close")}</Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}

// Export dialog
interface ExportDialogProps {
  open: boolean;
  onClose: () => void;
  sessionToken: string;
}

function ExportDialog({ open, onClose, sessionToken }: ExportDialogProps) {
  const { t } = useTranslation();
  const [step, setStep] = useState<ExportStep>("select");
  const [friends, setFriends] = useState<FriendInfo[]>([]);
  const [selections, setSelections] = useState<Record<number, FriendSelection>>({});
  const [mnemonic, setMnemonic] = useState("");
  const [password2, setPassword2] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [downloadBytes, setDownloadBytes] = useState<number[] | null>(null);

  useEffect(() => {
    if (!open) return;
    setStep("select");
    setError(null);
    setMnemonic("");
    setPassword2("");
    setDownloadBytes(null);
    invoke<FriendInfo[]>("list_friends", { sessionToken }).then((list) => {
      setFriends(list);
      const initial: Record<number, FriendSelection> = {};
      list.forEach((f) => { initial[f.id] = { profile: false, messages: false }; });
      setSelections(initial);
    });
  }, [open]);

  const toggleProfile = (id: number) => {
    setSelections((prev) => {
      const cur = prev[id];
      if (cur.profile) return { ...prev, [id]: { profile: false, messages: false } };
      return { ...prev, [id]: { ...cur, profile: true } };
    });
  };

  const toggleMessages = (id: number) => {
    setSelections((prev) => {
      const cur = prev[id];
      if (!cur.profile) return prev;
      return { ...prev, [id]: { ...cur, messages: !cur.messages } };
    });
  };

  const handleExport = async () => {
    setError(null);
    const wordList = mnemonic.trim().split(/\s+/);
    if (wordList.length !== 24) {
      setError(t("settings.recovery.mnemonic_length_error"));
      return;
    }
    if (!password2) {
      setError(t("settings.recovery.password2_required"));
      return;
    }

    const profileIds = Object.entries(selections)
      .filter(([, s]) => s.profile)
      .map(([id]) => Number(id));
    const messageIds = Object.entries(selections)
      .filter(([, s]) => s.messages)
      .map(([id]) => Number(id));

    setStep("exporting");
    try {
      const bytes = await invoke<number[]>("export_backup", {
        sessionToken,
        password1: wordList.join(" "),
        password2,
        friendIds: profileIds,
        messageFriendIds: messageIds,
      });
      setDownloadBytes(bytes);
      setStep("done");
    } catch (e) {
      setError(String(e));
      setStep("passwords");
    }
  };

  const handleDownload = () => {
    if (!downloadBytes) return;
    const blob = new Blob([new Uint8Array(downloadBytes)], { type: "application/octet-stream" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `zenth_backup_${Date.now()}.zbc`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const selectedCount = Object.values(selections).filter((s) => s.profile).length;

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-lg max-h-[90vh] overflow-y-auto">
        {step === "select" && (
          <>
            <DialogHeader>
              <DialogTitle className="text-foreground">{t("settings.recovery.export_select_title")}</DialogTitle>
              <DialogDescription className="text-muted-foreground">
                {t("settings.recovery.export_select_subtitle")}
              </DialogDescription>
            </DialogHeader>

            {friends.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-4">
                {t("settings.recovery.no_friends")}
              </p>
            ) : (
              <div className="space-y-2 max-h-72 overflow-y-auto pr-1">
                <div className="grid grid-cols-[1fr_auto_auto] gap-x-4 px-2 pb-1">
                  <span className="text-xs text-muted-foreground">{t("settings.recovery.col_contact")}</span>
                  <span className="text-xs text-muted-foreground text-center">{t("settings.recovery.col_profile")}</span>
                  <span className="text-xs text-muted-foreground text-center">{t("settings.recovery.col_messages")}</span>
                </div>
                {friends.map((f) => {
                  const sel = selections[f.id] ?? { profile: false, messages: false };
                  return (
                    <div key={f.id} className="grid grid-cols-[1fr_auto_auto] items-center gap-x-4 rounded-md border border-border px-3 py-2">
                      <span className="text-sm font-medium text-foreground truncate">{f.pseudo || f.username_hash.slice(0, 12)}</span>
                      <Checkbox
                        checked={sel.profile}
                        onCheckedChange={() => toggleProfile(f.id)}
                        className="mx-auto"
                      />
                      <Checkbox
                        checked={sel.messages}
                        disabled={!sel.profile}
                        onCheckedChange={() => toggleMessages(f.id)}
                        className="mx-auto"
                      />
                    </div>
                  );
                })}
              </div>
            )}

            <DialogFooter>
              <Button variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
              <Button onClick={() => setStep("passwords")}>
                {t("settings.recovery.export_next")} ({selectedCount})
              </Button>
            </DialogFooter>
          </>
        )}

        {step === "passwords" && (
          <>
            <DialogHeader>
              <DialogTitle className="text-foreground">{t("settings.recovery.export_passwords_title")}</DialogTitle>
              <DialogDescription className="text-muted-foreground">
                {t("settings.recovery.export_passwords_subtitle")}
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-4">
              <div className="space-y-2">
                <Label className="text-foreground">{t("settings.recovery.mnemonic_label")}</Label>
                <Textarea
                  value={mnemonic}
                  onChange={(e) => { setMnemonic(e.target.value); setError(null); }}
                  placeholder={t("settings.recovery.mnemonic_placeholder")}
                  className="font-mono text-sm resize-none"
                  rows={3}
                />
                <p className="text-xs text-muted-foreground">
                  {mnemonic.trim().split(/\s+/).filter(Boolean).length} / 24 {t("settings.recovery.words_count")}
                </p>
              </div>

              <div className="space-y-2">
                <Label className="text-foreground">{t("settings.recovery.password2_label")}</Label>
                <Input
                  type="password"
                  value={password2}
                  onChange={(e) => { setPassword2(e.target.value); setError(null); }}
                  placeholder={t("settings.recovery.password2_placeholder")}
                />
              </div>
            </div>

            {error && <p className="text-sm text-red-400">{error}</p>}

            <DialogFooter>
              <Button variant="outline" onClick={() => setStep("select")}>{t("common.back")}</Button>
              <Button onClick={handleExport}>{t("settings.recovery.export_btn")}</Button>
            </DialogFooter>
          </>
        )}

        {step === "exporting" && (
          <div className="flex flex-col items-center gap-3 py-8">
            <Loader2 className="h-8 w-8 animate-spin text-primary" />
            <p className="text-sm text-muted-foreground">{t("settings.recovery.exporting")}</p>
          </div>
        )}

        {step === "done" && (
          <>
            <DialogHeader>
              <DialogTitle className="text-foreground">{t("settings.recovery.export_done_title")}</DialogTitle>
              <DialogDescription className="text-muted-foreground">
                {t("settings.recovery.export_done_subtitle")}
              </DialogDescription>
            </DialogHeader>

            <div className="flex flex-col items-center gap-4 py-4">
              <FileArchive className="h-12 w-12 text-accent-secondary" />
              <Button className="gap-2 w-full" onClick={handleDownload}>
                <Download className="h-4 w-4" />
                {t("settings.recovery.download_btn")}
              </Button>
            </div>

            <DialogFooter>
              <Button variant="outline" onClick={onClose}>{t("common.close")}</Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}

// Import dialog
interface ImportDialogProps {
  open: boolean;
  onClose: () => void;
  sessionToken: string;
}

function ImportDialog({ open, onClose, sessionToken }: ImportDialogProps) {
  const { t } = useTranslation();
  const [step, setStep] = useState<ImportStep>("input");
  const [file, setFile] = useState<File | null>(null);
  const [mnemonic, setMnemonic] = useState("");
  const [password2, setPassword2] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ImportResult | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    setStep("input");
    setFile(null);
    setMnemonic("");
    setPassword2("");
    setError(null);
    setResult(null);
  }, [open]);

  const handleImport = async () => {
    setError(null);
    if (!file) { setError(t("settings.recovery.file_required")); return; }
    const wordList = mnemonic.trim().split(/\s+/);
    if (wordList.length !== 24) { setError(t("settings.recovery.mnemonic_length_error")); return; }
    if (!password2) { setError(t("settings.recovery.password2_required")); return; }

    setStep("importing");
    try {
      const ab = await file.arrayBuffer();
      const data = Array.from(new Uint8Array(ab));
      const res = await invoke<ImportResult>("import_backup", {
        sessionToken,
        data,
        password1: wordList.join(" "),
        password2,
      });
      setResult(res);
      setStep("done");
    } catch (e) {
      setError(String(e));
      setStep("input");
    }
  };

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-lg">
        {step === "input" && (
          <>
            <DialogHeader>
              <DialogTitle className="text-foreground">{t("settings.recovery.import_title")}</DialogTitle>
              <DialogDescription className="text-muted-foreground">
                {t("settings.recovery.import_subtitle")}
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-4">
              <div className="space-y-2">
                <Label className="text-foreground">{t("settings.recovery.file_label")}</Label>
                <div
                  className="flex cursor-pointer items-center justify-center gap-2 rounded-md border-2 border-dashed border-border bg-muted/20 px-4 py-6 text-sm text-muted-foreground transition-colors hover:border-primary/50 hover:bg-muted/40"
                  onClick={() => fileRef.current?.click()}
                >
                  <Upload className="h-5 w-5" />
                  {file ? (
                    <span className="font-medium text-foreground">{file.name}</span>
                  ) : (
                    <span>{t("settings.recovery.file_placeholder")}</span>
                  )}
                </div>
                <input
                  ref={fileRef}
                  type="file"
                  accept=".zbc"
                  className="hidden"
                  onChange={(e) => { setFile(e.target.files?.[0] ?? null); setError(null); }}
                />
              </div>

              <div className="space-y-2">
                <Label className="text-foreground">{t("settings.recovery.mnemonic_label")}</Label>
                <Textarea
                  value={mnemonic}
                  onChange={(e) => { setMnemonic(e.target.value); setError(null); }}
                  placeholder={t("settings.recovery.mnemonic_placeholder")}
                  className="font-mono text-sm resize-none"
                  rows={3}
                />
                <p className="text-xs text-muted-foreground">
                  {mnemonic.trim().split(/\s+/).filter(Boolean).length} / 24 {t("settings.recovery.words_count")}
                </p>
              </div>

              <div className="space-y-2">
                <Label className="text-foreground">{t("settings.recovery.password2_label")}</Label>
                <Input
                  type="password"
                  value={password2}
                  onChange={(e) => { setPassword2(e.target.value); setError(null); }}
                  placeholder={t("settings.recovery.password2_placeholder")}
                />
              </div>
            </div>

            {error && <p className="text-sm text-red-400">{error}</p>}

            <DialogFooter>
              <Button variant="outline" onClick={onClose}>{t("common.cancel")}</Button>
              <Button onClick={handleImport}>{t("settings.recovery.import_btn")}</Button>
            </DialogFooter>
          </>
        )}

        {step === "importing" && (
          <div className="flex flex-col items-center gap-3 py-8">
            <Loader2 className="h-8 w-8 animate-spin text-primary" />
            <p className="text-sm text-muted-foreground">{t("settings.recovery.importing")}</p>
          </div>
        )}

        {step === "done" && result && (
          <>
            <DialogHeader>
              <DialogTitle className="text-foreground">{t("settings.recovery.import_done_title")}</DialogTitle>
            </DialogHeader>

            <div className="space-y-3 py-2">
              <div className="rounded-md border border-border bg-muted/20 p-4 space-y-2 text-sm">
                <div className="flex justify-between">
                  <span className="text-muted-foreground">{t("settings.recovery.contacts_imported")}</span>
                  <span className="font-medium text-foreground">{result.contacts_imported}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">{t("settings.recovery.messages_imported")}</span>
                  <span className="font-medium text-foreground">{result.messages_imported}</span>
                </div>
                {result.has_recovery_key && (
                  <div className="flex items-center gap-2 text-green-500 pt-1">
                    <ShieldCheck className="h-4 w-4" />
                    <span>{t("settings.recovery.recovery_key_restored")}</span>
                  </div>
                )}
              </div>
            </div>

            <DialogFooter>
              <Button onClick={onClose}>{t("common.close")}</Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}

// Main section
export function RecoverySection() {
  const { sessionToken } = useAuth();
  const { t } = useTranslation();
  const [initialized, setInitialized] = useState<boolean | null>(null);
  const [initOpen, setInitOpen] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);

  const loadStatus = useCallback(async () => {
    if (!sessionToken) return;
    try {
      const status = await invoke<{ initialized: boolean }>("get_recovery_status", { sessionToken });
      setInitialized(status.initialized);
    } catch {
      setInitialized(false);
    }
  }, [sessionToken]);

  useEffect(() => { loadStatus(); }, [loadStatus]);

  if (!sessionToken) return null;

  return (
    <>
      <SettingsCard
        icon={KeyRound}
        title={t("settings.recovery.title")}
        description={t("settings.recovery.subtitle")}
      >
        {/* Status badge */}
        <div className="flex items-center gap-3">
          {initialized === null ? (
            <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
          ) : initialized ? (
            <div className="flex items-center gap-2 text-green-500">
              <ShieldCheck className="h-4 w-4" />
              <span className="text-sm font-medium">{t("settings.recovery.status_active")}</span>
            </div>
          ) : (
            <div className="flex items-center gap-2 text-yellow-500">
              <ShieldAlert className="h-4 w-4" />
              <span className="text-sm font-medium">{t("settings.recovery.status_inactive")}</span>
            </div>
          )}
        </div>

        {/* Init */}
        {initialized === false && (
          <>
            <div className="flex items-start gap-2 rounded-md border border-yellow-500/40 bg-yellow-500/10 px-3 py-2 text-sm text-yellow-600 dark:text-yellow-400">
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
              <span>{t("settings.recovery.not_initialized_warning")}</span>
            </div>
            <Button
              onClick={() => setInitOpen(true)}
              className="gap-2"
            >
              <KeyRound className="h-4 w-4" />
              {t("settings.recovery.init_btn")}
            </Button>
          </>
        )}

        {/* Export / Import (only when initialized) */}
        {initialized && (
          <>
            <Separator />
            <div className="flex flex-wrap gap-2">
              <Button variant="outline" className="gap-2" onClick={() => setExportOpen(true)}>
                <Download className="h-4 w-4" />
                {t("settings.recovery.export_btn_label")}
              </Button>
              <Button variant="outline" className="gap-2" onClick={() => setImportOpen(true)}>
                <Upload className="h-4 w-4" />
                {t("settings.recovery.import_btn_label")}
              </Button>
            </div>
          </>
        )}

        {/* Import always available */}
        {initialized === false && (
          <>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label className="text-foreground">{t("settings.recovery.import_btn_label")}</Label>
                <p className="text-sm text-muted-foreground">{t("settings.recovery.import_sub")}</p>
              </div>
              <Button variant="outline" size="sm" className="gap-2" onClick={() => setImportOpen(true)}>
                <Upload className="h-4 w-4" />
                {t("settings.recovery.import_btn_label")}
              </Button>
            </div>
          </>
        )}
      </SettingsCard>

      <InitDialog
        open={initOpen}
        onClose={() => setInitOpen(false)}
        onDone={() => { setInitialized(true); }}
        sessionToken={sessionToken}
      />
      <ExportDialog
        open={exportOpen}
        onClose={() => setExportOpen(false)}
        sessionToken={sessionToken}
      />
      <ImportDialog
        open={importOpen}
        onClose={() => { setImportOpen(false); loadStatus(); }}
        sessionToken={sessionToken}
      />
    </>
  );
}
