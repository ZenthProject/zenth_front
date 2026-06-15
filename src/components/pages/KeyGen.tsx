import { useEffect, useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useNavigate } from 'react-router-dom';
import { KeyRound, ShieldCheck } from 'lucide-react';

import { Textarea } from "@/components/ui/textarea";
import { TorButton } from '@/components/modules/tor';
import { useTranslation } from 'react-i18next';

type Point = { x: number; y: number; t: number };

const isMobile   = /android|iphone|ipad/i.test(navigator.userAgent);
const MAX_KEY    = 20000;
const CHUNK_SIZE = 500;
const MIN_MOBILE_PTS = 12;     // points capteur avant de démarrer sur mobile
const MOUSE_PTS_PER_CHUNK = 5; // points souris pour déclencher un chunk sur desktop

const yieldFrame = () => new Promise<void>(r => requestAnimationFrame(() => r()));

export default function KeyGen() {
  const { t }    = useTranslation();
  const navigate = useNavigate();

  const [displayedKey, setDisplayedKey] = useState('');
  const [progress, setProgress]         = useState(0); // 0-100
  const [done, setDone]                 = useState(false);

  const pointsRef    = useRef<Point[]>([]);
  const keyRef       = useRef('');
  const cancelRef    = useRef(false);
  // desktop : un seul chunk à la fois
  const chunkBusy    = useRef(false);

  const updateUI = (key: string) => {
    const pct = Math.round((key.length / MAX_KEY) * 100);
    setDisplayedKey(key);
    setProgress(pct);
  };

  // DESKTOP : déclenché par la souris, un chunk à la fois
  useEffect(() => {
    if (isMobile) return;

    const tryChunk = async () => {
      if (chunkBusy.current) return;
      if (keyRef.current.length >= MAX_KEY) return;
      if (pointsRef.current.length < MOUSE_PTS_PER_CHUNK) return;

      chunkBusy.current = true;
      const pts = pointsRef.current.splice(0);
      try {
        const chunk = await invoke<string>('generate_random_string_chunk', {
          points:        pts,
          chunkSize:     CHUNK_SIZE,
          browserEntropy: Array.from(crypto.getRandomValues(new Uint8Array(32))),
        });
        if (cancelRef.current) return;
        keyRef.current = (keyRef.current + chunk).slice(0, MAX_KEY);
        updateUI(keyRef.current);
        if (keyRef.current.length >= MAX_KEY) {
          setDone(true);
        }
      } catch { /* retry au prochain mouvement */ }
      chunkBusy.current = false;
    };

    cancelRef.current = false;

    const onMouse = (e: MouseEvent) => {
      if (keyRef.current.length >= MAX_KEY) return;
      pointsRef.current.push({ x: e.clientX, y: e.clientY, t: Date.now() });
      // déclenche dès qu'on a assez de points en attente
      if (pointsRef.current.length >= MOUSE_PTS_PER_CHUNK) tryChunk();
    };

    window.addEventListener('mousemove', onMouse);
    return () => {
      cancelRef.current = true;
      window.removeEventListener('mousemove', onMouse);
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // MOBILE : capteurs → seuil → boucle chunks avec flushSync
  useEffect(() => {
    if (!isMobile) return;

    cancelRef.current = false;
    let motionPts = 0;
    let started   = false;

    const startGeneration = async () => {
      if (started) return;
      started = true;

      while (!cancelRef.current && keyRef.current.length < MAX_KEY) {
        const pts = pointsRef.current.splice(0);
        if (pts.length === 0) pts.push({ x: 0, y: 0, t: Date.now() });

        try {
          const chunk = await invoke<string>('generate_random_string_chunk', {
            points:        pts,
            chunkSize:     CHUNK_SIZE,
            browserEntropy: Array.from(crypto.getRandomValues(new Uint8Array(32))),
          });
          if (cancelRef.current) return;
          keyRef.current = (keyRef.current + chunk).slice(0, MAX_KEY);
          updateUI(keyRef.current);
        } catch { /* retry */ }

        await yieldFrame(); // laisse le navigateur peindre entre chaque chunk
      }

      if (!cancelRef.current) setDone(true);
    };

    const addPoint = (x: number, y: number) => {
      pointsRef.current.push({ x, y, t: Date.now() });
      motionPts++;
      // progression de collecte reflétée avant le démarrage
      if (!started) {
        setProgress(Math.min(Math.round((motionPts / MIN_MOBILE_PTS) * 5), 5));
      }
      if (motionPts >= MIN_MOBILE_PTS) startGeneration();
    };

    const onMotion = (e: DeviceMotionEvent) => {
      const a = e.accelerationIncludingGravity;
      if (a) addPoint(Math.round((a.x ?? 0) * 1000), Math.round((a.y ?? 0) * 1000));
    };
    const onOrient = (e: DeviceOrientationEvent) =>
      addPoint(Math.round((e.beta ?? 0) * 100), Math.round((e.gamma ?? 0) * 100));
    const onTouch = (e: TouchEvent) =>
      Array.from(e.touches).forEach(t => addPoint(t.clientX, t.clientY));

    window.addEventListener('devicemotion',      onMotion, { passive: true });
    window.addEventListener('deviceorientation', onOrient, { passive: true });
    window.addEventListener('touchstart',        onTouch,  { passive: true });
    return () => {
      cancelRef.current = true;
      window.removeEventListener('devicemotion',      onMotion);
      window.removeEventListener('deviceorientation', onOrient);
      window.removeEventListener('touchstart',        onTouch);
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const isTouchDevice = window.matchMedia('(pointer: coarse)').matches;

  return (
    <div className="flex min-h-screen items-center justify-center bg-background py-8">
      <div className="w-full max-w-sm mx-4 space-y-8">

        <div className="text-center space-y-1.5">
          <h1 className="text-2xl font-bold text-foreground">
            {t("keygen.title")}
          </h1>
          <p className="text-sm text-muted-foreground">
            {isTouchDevice ? t("keygen.hint_touch") : t("keygen.hint_mouse")}
          </p>
        </div>

        <div className="space-y-2">
          <div className="w-full h-1 rounded-full overflow-hidden bg-muted">
            <div
              className="h-full bg-primary transition-all duration-100 ease-out"
              style={{ width: `${progress}%` }}
            />
          </div>
          <p className={`text-sm ${done ? 'text-success' : 'text-muted-foreground'}`}>
            {done
              ? t("keygen.entropy_complete")
              : t("keygen.entropy_progress", { percent: progress })}
          </p>
        </div>

        <div className="space-y-2">
          <label className="block text-xs font-medium text-muted-foreground uppercase tracking-wide">
            {t("keygen.key_preview")}
          </label>
          <Textarea
            readOnly
            value={displayedKey}
            className="font-mono text-xs bg-muted/50 border-border text-foreground resize-none h-32 overflow-y-auto"
          />
        </div>

        <div className={`transition-opacity duration-300 ${done ? 'opacity-100' : 'opacity-0 pointer-events-none'}`}>
          <TorButton
            onClick={() => navigate('/register', { state: { generatedKey: keyRef.current } })}
            isLoading={false}
            loadingText={t("keygen.validating")}
            Icon={done ? ShieldCheck : KeyRound}
          >
            {t("keygen.validate")}
          </TorButton>
        </div>

      </div>
    </div>
  );
}
