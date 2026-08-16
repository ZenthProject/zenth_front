import { useEffect, useState, useRef, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useNavigate } from 'react-router-dom';
import {
  KeyRound, ShieldCheck, CheckCircle2, RefreshCw, Copy, Check, QrCode,
} from 'lucide-react';
import { QRCodeSVG } from 'qrcode.react';
import { QRScanner } from '@/components/modules/QRScanner';
import { TorButton } from '@/components/modules/tor';
import { useTranslation } from 'react-i18next';
import { useAuth } from '@/contexts/AuthContext';

// ─── types ────────────────────────────────────────────────────────────────────

// keygen  : user generates entropy → key produced
// setup   : register(key) + login + publish_pairing_keys in background
// show_qr : display pairing QR
// scan_return / verifying / success / error : rest of pairing flow
type Step = 'keygen' | 'setup' | 'show_qr' | 'scan_return' | 'verifying' | 'success' | 'error';
type Point = { x: number; y: number; t: number };

// ─── constants ────────────────────────────────────────────────────────────────

const KEYGEN_TARGET = 1024;
const CHUNK_SIZE    = 256;
const MOUSE_PTS_REQ = 4;
const isMobile      = /android|iphone|ipad/i.test(navigator.userAgent);

// ─── temp credential generators ───────────────────────────────────────────────

function genTempPassword(): string {
  const upper   = 'BCDEFGHJKLMNPQRSTUVWXYZ';
  const lower   = 'bcdefghjkmnpqrstuvwxyz';
  const digits  = '23456789';
  const special = '!@#$%^*_+=';
  const all     = upper + lower + digits + special;
  const buf     = new Uint8Array(32);
  crypto.getRandomValues(buf);
  const parts = [
    upper[buf[0] % upper.length],   upper[buf[1] % upper.length],   upper[buf[2] % upper.length],
    lower[buf[3] % lower.length],   lower[buf[4] % lower.length],   lower[buf[5] % lower.length],
    digits[buf[6] % digits.length], digits[buf[7] % digits.length], digits[buf[8] % digits.length],
    special[buf[9] % special.length], special[buf[10] % special.length], special[buf[11] % special.length],
  ];
  for (let i = 12; i < 24; i++) parts.push(all[buf[i] % all.length]);
  for (let i = parts.length - 1; i > 0; i--) {
    const j = buf[i % buf.length] % (i + 1);
    [parts[i], parts[j]] = [parts[j], parts[i]];
  }
  return parts.join('');
}

function genTempUsername(): string {
  const buf = new Uint8Array(8);
  crypto.getRandomValues(buf);
  return 'z' + Array.from(buf).map(b => b.toString(16).padStart(2, '0')).join('');
}

function classifyError(e: unknown, networkKey: string, t: (k: string) => string): string {
  const msg = String(e);
  if (
    msg.includes('Transport') || msg.includes('transport') ||
    msg.includes('Network error') || msg.includes('SERVER_UNAVAILABLE') ||
    msg.includes('HTTP request failed') || msg.includes('Connection refused')
  ) return t(networkKey);
  if (msg.includes('sync_account_conflict')) return t('settings.synchronization.error_account_conflict');
  return msg;
}

// ─── helpers ──────────────────────────────────────────────────────────────────

function Spinner({ label }: { label: string }) {
  return (
    <div className="flex flex-col items-center gap-3 py-8">
      <RefreshCw className="w-6 h-6 text-primary animate-spin" />
      <p className="text-sm text-muted-foreground text-center">{label}</p>
    </div>
  );
}

function QrBlock({ payload, hint }: { payload: string; hint?: string }) {
  const [copied, setCopied] = useState(false);
  const copy = () => {
    navigator.clipboard?.writeText(payload).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    });
  };
  return (
    <div className="flex flex-col items-center gap-3 w-full">
      <div className="mx-auto w-full max-w-[200px] aspect-square bg-white rounded-xl p-2.5 shadow-sm">
        <QRCodeSVG value={payload || ' '} size={512} level="M" style={{ width: '100%', height: 'auto', display: 'block' }} />
      </div>
      {hint && <p className="text-xs text-muted-foreground text-center opacity-70">{hint}</p>}
      <button
        onClick={copy}
        className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors border border-border rounded-md px-3 py-1.5"
      >
        {copied
          ? <><Check className="w-3 h-3 text-success" /> Copié</>
          : <><Copy className="w-3 h-3" /> Copier le code</>}
      </button>
    </div>
  );
}

// ─── KeyGen visual ────────────────────────────────────────────────────────────
// The generated key (1024 chars from user entropy) is passed to onComplete
// so it can be forwarded to the Rust `register` command as additional entropy.

function KeyGenVisual({ onComplete }: { onComplete: (key: string) => void }) {
  const { t } = useTranslation();
  const [displayedKey, setDisplayedKey] = useState('');
  const [progress, setProgress]         = useState(0);
  const [keygenDone, setKeygenDone]     = useState(false);

  const keyRef    = useRef('');
  const aliveRef  = useRef(true);
  const pointsRef = useRef<Point[]>([]);
  const chunkBusy = useRef(false);

  useEffect(() => {
    aliveRef.current = true;
    return () => { aliveRef.current = false; };
  }, []);

  // Fire onComplete(key) 500 ms after entropy collection is done
  useEffect(() => {
    if (!keygenDone) return;
    const key = keyRef.current;
    const tid = setTimeout(() => onComplete(key), 500);
    return () => clearTimeout(tid);
  }, [keygenDone, onComplete]);

  const markDone = () => {
    if (!aliveRef.current) return;
    setProgress(100);
    setKeygenDone(true);
  };

  // Desktop entropy
  useEffect(() => {
    if (isMobile) return;
    const tryChunk = async () => {
      if (chunkBusy.current || keyRef.current.length >= KEYGEN_TARGET) return;
      if (pointsRef.current.length < MOUSE_PTS_REQ) return;
      chunkBusy.current = true;
      const pts = pointsRef.current.splice(0);
      try {
        const chunk = await invoke<string>('generate_random_string_chunk', {
          points: pts, chunkSize: CHUNK_SIZE,
          browserEntropy: Array.from(crypto.getRandomValues(new Uint8Array(32))),
        });
        if (!aliveRef.current) return;
        keyRef.current = (keyRef.current + chunk).slice(0, KEYGEN_TARGET);
        setDisplayedKey(keyRef.current);
        setProgress(Math.round((keyRef.current.length / KEYGEN_TARGET) * 100));
        if (keyRef.current.length >= KEYGEN_TARGET) markDone();
      } catch { /* retry on next move */ }
      chunkBusy.current = false;
    };
    const onMouse = (e: MouseEvent) => {
      if (keyRef.current.length >= KEYGEN_TARGET) return;
      pointsRef.current.push({ x: e.clientX, y: e.clientY, t: Date.now() });
      if (pointsRef.current.length >= MOUSE_PTS_REQ) tryChunk();
    };
    window.addEventListener('mousemove', onMouse);
    return () => window.removeEventListener('mousemove', onMouse);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Mobile entropy
  useEffect(() => {
    if (!isMobile) return;
    let running = true;
    const run = async () => {
      while (running && aliveRef.current && keyRef.current.length < KEYGEN_TARGET) {
        const pts = pointsRef.current.splice(0);
        if (!pts.length) pts.push({ x: 0, y: 0, t: Date.now() });
        try {
          const chunk = await invoke<string>('generate_random_string_chunk', {
            points: pts, chunkSize: CHUNK_SIZE,
            browserEntropy: Array.from(crypto.getRandomValues(new Uint8Array(32))),
          });
          if (!aliveRef.current) return;
          keyRef.current = (keyRef.current + chunk).slice(0, KEYGEN_TARGET);
          setDisplayedKey(keyRef.current);
          setProgress(Math.round((keyRef.current.length / KEYGEN_TARGET) * 100));
        } catch { /* retry */ }
        await new Promise(r => requestAnimationFrame(r));
      }
      if (aliveRef.current) markDone();
    };
    const addPt = (x: number, y: number) => pointsRef.current.push({ x, y, t: Date.now() });
    const onMotion  = (e: DeviceMotionEvent) => {
      const a = e.accelerationIncludingGravity;
      if (a) addPt(Math.round((a.x ?? 0) * 1000), Math.round((a.y ?? 0) * 1000));
    };
    const onOrient  = (e: DeviceOrientationEvent) =>
      addPt(Math.round((e.beta ?? 0) * 100), Math.round((e.gamma ?? 0) * 100));
    const onTouch   = (e: TouchEvent) =>
      Array.from(e.touches).forEach(t => addPt(t.clientX, t.clientY));
    window.addEventListener('devicemotion',      onMotion, { passive: true });
    window.addEventListener('deviceorientation', onOrient, { passive: true });
    window.addEventListener('touchstart',        onTouch,  { passive: true });
    run();
    return () => {
      running = false;
      window.removeEventListener('devicemotion',      onMotion);
      window.removeEventListener('deviceorientation', onOrient);
      window.removeEventListener('touchstart',        onTouch);
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const isTouchDevice = useMemo(() => window.matchMedia('(pointer: coarse)').matches, []);

  return (
    <div className="space-y-4 py-2">
      <div className="text-center space-y-1">
        <KeyRound className="w-8 h-8 text-primary mx-auto" />
        <p className="text-sm font-medium text-foreground">{t('sync.keygen_title')}</p>
        <p className="text-xs text-muted-foreground">
          {isTouchDevice ? t('sync.keygen_hint_touch') : t('sync.keygen_hint_mouse')}
        </p>
      </div>
      <div className="space-y-1.5">
        <div className="w-full h-1 rounded-full overflow-hidden bg-muted">
          <div
            className="h-full bg-primary transition-all duration-100 ease-out"
            style={{ width: `${progress}%` }}
          />
        </div>
        <p className="text-xs text-muted-foreground text-right tabular-nums">{progress}%</p>
      </div>
      <div
        className="font-mono text-[10px] leading-tight bg-muted/50 border border-border rounded-md p-2.5 h-20 overflow-hidden text-muted-foreground break-all"
        aria-label={t('sync.keygen_preview_label')}
      >
        {displayedKey || <span className="opacity-40">···</span>}
      </div>
    </div>
  );
}

// ─── inner flow (remountable on retry) ────────────────────────────────────────

function SyncFlow({ onRetry }: { onRetry: () => void }) {
  const { t }                              = useTranslation();
  const navigate                           = useNavigate();
  const { sessionToken, login: authLogin } = useAuth();

  const [step, setStep]                     = useState<Step>('keygen');
  const [qrPayload, setQrPayload]           = useState<string | null>(null);
  const [error, setError]                   = useState<string | null>(null);
  const [effectiveUsername, setEffectiveUsername] = useState('');

  const tempTokenRef    = useRef('');
  const tempPasswordRef = useRef('');
  const tokenRef        = useRef(sessionToken ?? '');
  const generatedKeyRef = useRef('');   // key from KeyGenVisual
  const setupRanRef     = useRef(false);

  // ── Setup: runs after keygen finishes, uses the entropy key ─────────────────

  useEffect(() => {
    if (step !== 'setup' || setupRanRef.current) return;
    setupRanRef.current = true;

    (async () => {
      try {
        let token    = sessionToken ?? '';
        let password = '';

        if (!token) {
          const user = genTempUsername();
          password   = genTempPassword();
          tempPasswordRef.current = password;

          // Pass the user-generated entropy key to Rust register
          await invoke('register', {
            username:     user,
            password,
            generatedKey: generatedKeyRef.current || null,
          });
          token = await invoke<string>('login', { username: user, password });
          tempTokenRef.current = token;
          tokenRef.current     = token;
        }

        const qr = await invoke<string>('publish_pairing_keys', {
          sessionToken: token,
          password: password || tempPasswordRef.current,
        });
        setQrPayload(qr);
        setStep('show_qr');
      } catch (e) {
        setError(classifyError(e, 'sync.error_network', t));
        setStep('error');
      }
    })();
  }, [step, sessionToken, t]);

  // ── Keygen complete — store the entropy key and transition to setup ──────────

  const handleKeygenComplete = useCallback((key: string) => {
    generatedKeyRef.current = key;
    setStep('setup');
  }, []);

  // ── Scan return QR ───────────────────────────────────────────────────────────

  const handleScanReturnQr = async (payload: string) => {
    const token = tokenRef.current;
    setError(null);
    setStep('verifying');
    try {
      const result = await invoke<{
        pubkey: string;
        username_hash_hex: string;
        trusted_username: string;
      }>('verify_pairing_qr', { sessionToken: token, qrJson: payload });

      const syncStr = await invoke<string>('fetch_sync_key', {
        sessionToken:             token,
        dilithiumPubkeyTelBase64: result.pubkey,
        trustedUsernameHashHex:   result.username_hash_hex ?? '',
        trustedUsername:          result.trusted_username ?? '',
      });

      let uname = result.trusted_username ?? '';
      try { const p = JSON.parse(syncStr); if (p?.effective_username) uname = p.effective_username; } catch {}
      setEffectiveUsername(uname);

      invoke('relay_pull_messages', { sessionToken: token }).catch(() => {});
      setStep('success');
    } catch (e) {
      setError(classifyError(e, 'sync.error_network', t));
      setStep('error');
    }
  };

  // ── Post-sync login ───────────────────────────────────────────────────────────

  const handleContinue = () => {
    const token = tokenRef.current;
    if (token && effectiveUsername) {
      authLogin(effectiveUsername, token, tempPasswordRef.current || undefined);
      navigate('/chat');
    } else {
      navigate('/login');
    }
  };

  // ── Step indicator labels ─────────────────────────────────────────────────────

  const stepLabel: Partial<Record<Step, string>> = {
    show_qr:     t('sync.step_qr'),
    scan_return: t('sync.step_scan'),
    verifying:   t('sync.step_verify'),
    success:     t('sync.step_done'),
    error:       t('sync.step_done'),
  };

  // s(target) → '' when active, 'hidden' otherwise
  const s = (target: Step) => step === target ? '' : 'hidden';

  return (
    <div className="w-[340px] sm:w-[400px] space-y-6">

      {/* Header */}
      <div className="text-center space-y-1.5">
        <div className="flex items-center justify-center gap-2 mb-1">
          <div className="flex items-center justify-center w-10 h-10 rounded-xl bg-primary/10">
            <RefreshCw className="w-5 h-5 text-primary" />
          </div>
        </div>
        <h1 className="text-2xl font-bold text-foreground">{t('sync.title')}</h1>
        <p className="text-sm text-muted-foreground">{t('sync.subtitle')}</p>
      </div>

      {/* Step indicator — always in DOM */}
      <div className={`flex items-center justify-center ${!stepLabel[step] ? 'hidden' : ''}`}>
        <span className="text-xs text-muted-foreground border border-border rounded-full px-3 py-1">
          {stepLabel[step]}
        </span>
      </div>

      {/*
        All panels always in DOM — only hidden class is toggled.
        Prevents concurrent removeChild + appendChild in the same React commit
        which would crash WebKitGTK.
      */}
      <div className="bg-card border border-border rounded-2xl p-6 shadow-sm">

        {/* keygen: entropy collection → key forwarded to register */}
        <div className={s('keygen')}>
          <KeyGenVisual onComplete={handleKeygenComplete} />
        </div>

        {/* setup: spinner while register + login + publish_pairing_keys runs */}
        <div className={s('setup')}>
          <Spinner label={t('sync.keygen_server_wait')} />
        </div>

        {/* show_qr */}
        <div className={`space-y-4 ${s('show_qr')}`}>
          <p className="text-sm text-muted-foreground text-center">
            {t('sync.show_qr_hint')}
          </p>
          <QrBlock
            payload={qrPayload ?? ''}
            hint={t('settings.synchronization.copy_qr_hint')}
          />
          <TorButton
            onClick={() => setStep('scan_return')}
            isLoading={false}
            loadingText=""
            Icon={QrCode}
          >
            {t('settings.synchronization.scan_return_btn')}
          </TorButton>
        </div>

        {/* scan_return — QRScanner only mounted when active to avoid camera running idle */}
        <div className={`space-y-4 ${s('scan_return')}`}>
          <p className="text-sm text-muted-foreground text-center">
            {t('settings.synchronization.scan_return_instruction')}
          </p>
          {step === 'scan_return' && (
            <QRScanner onScan={handleScanReturnQr} onError={e => setError(e)} />
          )}
          <p className={`text-xs text-destructive text-center ${error ? '' : 'hidden'}`}>
            {error}
          </p>
        </div>

        {/* verifying */}
        <div className={s('verifying')}>
          <Spinner label={t('settings.synchronization.verifying')} />
        </div>

        {/* success */}
        <div className={`flex flex-col items-center gap-5 py-4 ${s('success')}`}>
          <CheckCircle2 className="w-12 h-12 text-success" />
          <div className="text-center space-y-1">
            <p className="text-sm font-medium text-success">{t('sync.success_title')}</p>
            <p className="text-xs text-muted-foreground">{t('sync.success_detail')}</p>
          </div>
          <TorButton
            onClick={handleContinue}
            isLoading={false}
            loadingText=""
            Icon={ShieldCheck}
          >
            {t('sync.goto_chat')}
          </TorButton>
        </div>

        {/* error */}
        <div className={`space-y-4 ${s('error')}`}>
          <p className="text-sm text-destructive text-center">{error}</p>
          <TorButton
            onClick={onRetry}
            isLoading={false}
            loadingText=""
            Icon={RefreshCw}
          >
            {t('sync.retry')}
          </TorButton>
        </div>

      </div>

      {/* Back to login — hidden once past keygen */}
      <p className={`text-center text-sm text-muted-foreground ${s('keygen')}`}>
        <button
          onClick={() => navigate('/login')}
          className="text-primary hover:text-accent-secondary font-medium underline transition-colors"
        >
          {t('sync.back_to_login')}
        </button>
      </p>

    </div>
  );
}

// ─── outer wrapper: remounts SyncFlow on retry ────────────────────────────────

export default function Sync() {
  const [attempt, setAttempt] = useState(0);
  return (
    <div className="flex min-h-screen items-center justify-center bg-background py-8">
      <SyncFlow key={attempt} onRetry={() => setAttempt(a => a + 1)} />
    </div>
  );
}
