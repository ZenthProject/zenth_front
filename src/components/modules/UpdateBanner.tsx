import { Download, RefreshCw, ShieldOff, CheckCircle2, XCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useUpdate } from "@/contexts/UpdateContext";
import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";

function cmpVersion(a: string, b: string): number {
  const parse = (v: string) => v.split(".").map(n => parseInt(n, 10) || 0);
  const [a0, a1, a2] = parse(a);
  const [b0, b1, b2] = parse(b);
  return a0 !== b0 ? a0 - b0 : a1 !== b1 ? a1 - b1 : a2 - b2;
}

export default function UpdateBanner() {
  const {
    isOutdated,
    latestVersion,
    downloadProgress,
    error,
    startDownload,
    installUpdate,
  } = useUpdate();

  const [currentVersion, setCurrentVersion] = useState<string>("");

  useEffect(() => {
    getVersion().then(setCurrentVersion).catch(() => {});
  }, []);

  if (!isOutdated && downloadProgress === null && !error) return null;

  const isDownloading = downloadProgress !== null && downloadProgress < 100;
  const isReady = downloadProgress === 100;

  // Mise à jour réelle disponible : la version cible est strictement plus récente
  const canUpgrade =
    latestVersion && currentVersion
      ? cmpVersion(latestVersion, currentVersion) > 0
      : false;

  const barColor = error
    ? "bg-destructive/10 border-b border-destructive/40 text-destructive"
    : isReady
    ? "bg-green-500/15 border-b border-green-500/40 text-green-400"
    : "bg-amber-500/10 border-b border-amber-500/40 text-amber-400";

  const lineColor = error
    ? "bg-destructive"
    : isReady
    ? "bg-green-500"
    : "bg-amber-500 animate-pulse";

  // VERSION_OUTDATED reçu mais pas de vraie mise à jour → problème serveur, rien à faire côté client
  if (!error && !isDownloading && !isReady && !canUpgrade) return null;

  return (
    <div className="relative w-full overflow-hidden">
      <div className={`relative flex items-center gap-3 px-4 py-2.5 text-sm font-medium ${barColor}`}>
        <div className={`absolute top-0 left-0 h-0.5 w-full ${lineColor}`} />

        <div className="shrink-0">
          {error && <XCircle className="h-4 w-4" />}
          {!error && canUpgrade && !isDownloading && !isReady && (
            <ShieldOff className="h-4 w-4 animate-pulse" />
          )}
          {isDownloading && <Download className="h-4 w-4 animate-bounce" />}
          {isReady && <CheckCircle2 className="h-4 w-4" />}
        </div>

        <div className="flex flex-1 items-center gap-2 min-w-0">
          {error && (
            <span className="truncate text-xs">{error}</span>
          )}

          {!error && canUpgrade && !isDownloading && !isReady && (
            <span>
              Mise à jour requise :{" "}
              <span className="font-mono">v{currentVersion}</span>
              {" → "}
              <span className="font-mono">v{latestVersion}</span>
            </span>
          )}

          {isDownloading && (
            <div className="flex flex-1 items-center gap-3 min-w-0">
              <span className="shrink-0">
                Téléchargement v{latestVersion} via réseau Zenth…
              </span>
              <div className="flex-1 min-w-0 max-w-xs">
                <div className="h-1.5 w-full rounded-full bg-black/20">
                  <div
                    className="h-1.5 rounded-full bg-amber-400 transition-all duration-200"
                    style={{ width: `${downloadProgress}%` }}
                  />
                </div>
              </div>
              <span className="font-mono text-xs opacity-70 shrink-0">
                {downloadProgress}%
              </span>
            </div>
          )}

          {isReady && (
            <span>
              Mise à jour v{latestVersion} vérifiée: redémarrage requis.
            </span>
          )}
        </div>

        <div className="shrink-0">
          {!error && canUpgrade && !isDownloading && !isReady && (
            <Button
              size="sm"
              variant="outline"
              onClick={startDownload}
              className="h-7 text-xs font-semibold border border-amber-500/60 text-amber-400 hover:bg-amber-500/10"
            >
              <Download className="h-3 w-3 mr-1.5" />
              Mettre à jour
            </Button>
          )}

          {isReady && (
            <Button
              size="sm"
              onClick={installUpdate}
              className="h-7 text-xs font-semibold bg-green-500 hover:bg-green-600 text-white"
            >
              <RefreshCw className="h-3 w-3 mr-1.5" />
              Installer et redémarrer
            </Button>
          )}

          {error && (
            <Button
              size="sm"
              variant="outline"
              onClick={startDownload}
              className="h-7 text-xs font-semibold border border-destructive/60 text-destructive hover:bg-destructive/10"
            >
              Réessayer
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
