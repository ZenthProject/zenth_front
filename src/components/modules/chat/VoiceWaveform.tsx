import { useEffect, useRef } from 'react';

interface VoiceWaveformProps {
  analyserRef: React.MutableRefObject<AnalyserNode | null>;
  duration: number;
  formatDuration: (s: number) => string;
}

export function VoiceWaveform({ analyserRef, duration, formatDuration }: VoiceWaveformProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rafRef    = useRef<number>(0);

  useEffect(() => {
    const canvas   = canvasRef.current;
    const analyser = analyserRef.current;
    if (!canvas || !analyser) return;

    const ctx  = canvas.getContext('2d')!;
    const data = new Uint8Array(analyser.frequencyBinCount);

    const draw = () => {
      rafRef.current = requestAnimationFrame(draw);
      analyser.getByteFrequencyData(data);

      const w = canvas.width;
      const h = canvas.height;
      ctx.clearRect(0, 0, w, h);

      const barCount = data.length;
      const barW     = (w / barCount) * 0.7;
      const gap      = (w / barCount) * 0.3;

      data.forEach((val, i) => {
        const barH  = (val / 255) * h;
        const x     = i * (barW + gap);
        const alpha = 0.5 + (val / 255) * 0.5;

        ctx.fillStyle = `rgba(139, 92, 246, ${alpha})`; // violet-500
        ctx.beginPath();
        ctx.roundRect(x, h - barH, barW, Math.max(barH, 2), 2);
        ctx.fill();
      });
    };

    draw();
    return () => cancelAnimationFrame(rafRef.current);
  }, [analyserRef]);

  return (
    <div className="flex items-center gap-2 flex-1 min-w-0">
      <div className="flex items-center gap-1 shrink-0">
        <span className="w-2 h-2 rounded-full bg-primary animate-pulse inline-block" />
        <span className="text-xs text-primary font-mono">{formatDuration(duration)}</span>
      </div>
      <canvas
        ref={canvasRef}
        width={200}
        height={32}
        className="flex-1 h-8 rounded"
      />
    </div>
  );
}
