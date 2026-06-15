import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

interface QRScannerProps {
  onScan: (data: string) => void;
  onError?: (err: string) => void;
  pasteDescription?: string;
  pastePlaceholder?: string;
  pasteHint?: string;
}

declare global {
  interface Window {
    BarcodeDetector?: new (opts: { formats: string[] }) => {
      detect(source: ImageBitmapSource): Promise<Array<{ rawValue: string }>>;
    };
  }
}

function PasteScanner({ onScan, hint, description, placeholder }: {
  onScan: (data: string) => void;
  hint?: string;
  description?: string;
  placeholder?: string;
}) {
  const { t } = useTranslation();
  const [value, setValue] = useState("");

  return (
    <div className="flex flex-col gap-3 w-full">
      {hint && <p className="text-xs text-amber-400 text-center">{hint}</p>}
      <p className="text-sm text-muted-foreground text-center">
        {description ?? t("qr_scanner.paste_default_desc")}
      </p>
      <Textarea
        className="font-mono text-xs h-24 resize-none"
        placeholder={placeholder ?? '{"pid":"...","h":"...","v":"1"}'}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
            const trimmed = value.trim();
            if (trimmed) onScan(trimmed);
          }
        }}
        autoFocus
      />
      <Button size="sm" className="w-full" disabled={!value.trim()}
        onClick={() => { const trimmed = value.trim(); if (trimmed) onScan(trimmed); }}>
        {t("qr_scanner.validate")}
      </Button>
    </div>
  );
}

function CameraScanner({ onScan, onError: _onError, pasteDescription, pastePlaceholder }: QRScannerProps) {
  const { t } = useTranslation();
  const videoRef = useRef<HTMLVideoElement>(null);
  const [status, setStatus] = useState<"init" | "scanning" | "no_camera">("init");
  const streamRef = useRef<MediaStream | null>(null);
  const rafRef = useRef<number>(0);
  const scannedRef = useRef(false);

  useEffect(() => {
    const detector = new window.BarcodeDetector!({ formats: ["qr_code"] });

    navigator.mediaDevices
      .getUserMedia({ video: { facingMode: "environment" } })
      .then((stream) => {
        streamRef.current = stream;
        if (!videoRef.current) return;
        videoRef.current.srcObject = stream;
        videoRef.current.play();
        setStatus("scanning");

        const tick = async () => {
          if (scannedRef.current || !videoRef.current) return;
          try {
            const results = await detector.detect(videoRef.current);
            if (results.length > 0 && results[0].rawValue) {
              scannedRef.current = true;
              stopStream();
              onScan(results[0].rawValue);
              return;
            }
          } catch { /* frame pas encore prête */ }
          rafRef.current = requestAnimationFrame(tick);
        };
        rafRef.current = requestAnimationFrame(tick);
      })
      .catch(() => setStatus("no_camera"));

    return () => { stopStream(); cancelAnimationFrame(rafRef.current); };
  }, []);

  const stopStream = () => {
    streamRef.current?.getTracks().forEach((t) => t.stop());
    streamRef.current = null;
  };

  if (status === "no_camera") {
    return (
      <PasteScanner
        onScan={onScan}
        hint={t("qr_scanner.camera_unavailable")}
        description={pasteDescription}
        placeholder={pastePlaceholder}
      />
    );
  }

  return (
    <div className="flex flex-col items-center gap-2 w-full">
      <div className="relative w-48 h-48 rounded-lg overflow-hidden bg-black">
        <video ref={videoRef} className="w-full h-full object-cover" muted playsInline />
        <div className="absolute inset-0 border-2 border-white/30 rounded-lg pointer-events-none">
          <div className="absolute top-2 left-2 w-5 h-5 border-t-2 border-l-2 border-white" />
          <div className="absolute top-2 right-2 w-5 h-5 border-t-2 border-r-2 border-white" />
          <div className="absolute bottom-2 left-2 w-5 h-5 border-b-2 border-l-2 border-white" />
          <div className="absolute bottom-2 right-2 w-5 h-5 border-b-2 border-r-2 border-white" />
        </div>
      </div>
      <p className="text-xs text-muted-foreground text-center">
        {status === "init" ? t("qr_scanner.camera_starting") : t("qr_scanner.camera_aim")}
      </p>
    </div>
  );
}

export function QRScanner({ onScan, onError, pasteDescription, pastePlaceholder, pasteHint }: QRScannerProps) {
  if (typeof window === "undefined" || !window.BarcodeDetector) {
    return (
      <PasteScanner
        onScan={onScan}
        description={pasteDescription}
        placeholder={pastePlaceholder}
        hint={pasteHint}
      />
    );
  }
  return (
    <CameraScanner
      onScan={onScan}
      onError={onError}
      pasteDescription={pasteDescription}
      pastePlaceholder={pastePlaceholder}
    />
  );
}
