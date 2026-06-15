import { useEffect, useRef, useState } from "react";
import { Play, Pause } from "lucide-react";
import { cn } from "@/lib/utils";

interface AudioPlayerProps {
  src: string;
  mime?: string;
  isOwn?: boolean;
}

export function AudioPlayer({ src, mime = "audio/webm", isOwn = false }: AudioPlayerProps) {
  const audioRef  = useRef<HTMLAudioElement>(null);
  const urlRef    = useRef<string | null>(null);
  const [playing,  setPlaying]  = useState(false);
  const [progress, setProgress] = useState(0);
  const [duration, setDuration] = useState(0);
  const [blobUrl,  setBlobUrl]  = useState<string | null>(null);

  useEffect(() => {
    let url: string;
    if (src.startsWith("blob:") || src.startsWith("http")) {
      url = src;
    } else {
      const bin = atob(src.replace(/[\s\r\n]/g, ""));
      const buf = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) buf[i] = bin.charCodeAt(i);
      url = URL.createObjectURL(new Blob([buf], { type: mime }));
      urlRef.current = url;
    }
    setBlobUrl(url);
    return () => {
      if (urlRef.current) { URL.revokeObjectURL(urlRef.current); urlRef.current = null; }
    };
  }, [src, mime]);

  const toggle = () => {
    const a = audioRef.current;
    if (!a) return;
    if (a.paused) a.play().catch(() => {});
    else          a.pause();
  };

  const fmt = (s: number) =>
    `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

  const pct = duration ? (progress / duration) * 100 : 0;

  return (
    <div className={cn(
      "flex items-center gap-3 rounded-2xl px-3 py-2.5 min-w-[200px] max-w-[260px]",
      isOwn
        ? "bg-primary/20 border border-primary/30"
        : "bg-accent-secondary/10 border border-accent-secondary/20"
    )}>
      {blobUrl && (
        <audio
          ref={audioRef}
          src={blobUrl}
          preload="auto"
          onPlay={()           => setPlaying(true)}
          onPause={()          => setPlaying(false)}
          onEnded={()          => { setPlaying(false); setProgress(0); if (audioRef.current) audioRef.current.currentTime = 0; }}
          onLoadedMetadata={()   => { const d = audioRef.current?.duration; if (d && isFinite(d)) setDuration(d); }}
          onDurationChange={()   => { const d = audioRef.current?.duration; if (d && isFinite(d)) setDuration(d); }}
          onCanPlay={()          => { const d = audioRef.current?.duration; if (d && isFinite(d)) setDuration(d); }}
          onTimeUpdate={()      => setProgress(audioRef.current?.currentTime ?? 0)}
          style={{ display: "none" }}
        />
      )}

      {/* Bouton play/pause */}
      <button
        onClick={toggle}
        className={cn(
          "shrink-0 w-9 h-9 rounded-full flex items-center justify-center transition-all",
          "shadow-md active:scale-95",
          isOwn
            ? "bg-primary text-primary-foreground hover:bg-primary/90"
            : "bg-accent-secondary text-white hover:bg-accent-secondary/90"
        )}
      >
        {playing
          ? <Pause className="w-4 h-4 fill-current" />
          : <Play  className="w-4 h-4 fill-current translate-x-0.5" />
        }
      </button>

      {/* Barre + durée */}
      <div className="flex flex-col gap-1.5 flex-1 min-w-0">
        {/* Barre de progression custom */}
        <div className="relative h-1.5 rounded-full bg-white/10 cursor-pointer overflow-hidden"
          onClick={(e) => {
            const rect = e.currentTarget.getBoundingClientRect();
            const ratio = (e.clientX - rect.left) / rect.width;
            const t = ratio * (duration || 0);
            if (audioRef.current) audioRef.current.currentTime = t;
            setProgress(t);
          }}
        >
          <div
            className={cn(
              "absolute inset-y-0 left-0 rounded-full transition-all",
              isOwn ? "bg-primary" : "bg-accent-secondary"
            )}
            style={{ width: `${pct}%` }}
          />
          {/* Curseur */}
          <div
            className={cn(
              "absolute top-1/2 -translate-y-1/2 w-2.5 h-2.5 rounded-full shadow",
              isOwn ? "bg-primary" : "bg-accent-secondary"
            )}
            style={{ left: `calc(${pct}% - 5px)` }}
          />
        </div>

        {/* Temps */}
        <div className="flex justify-between text-[10px] font-mono text-white/50">
          <span>{fmt(progress)}</span>
          <span>{fmt(duration)}</span>
        </div>
      </div>
    </div>
  );
}
