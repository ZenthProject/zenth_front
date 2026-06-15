import { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

function cmpVersion(a: string, b: string): number {
  const parse = (v: string) => v.split(".").map(n => parseInt(n, 10) || 0);
  const [a0, a1, a2] = parse(a);
  const [b0, b1, b2] = parse(b);
  return a0 !== b0 ? a0 - b0 : a1 !== b1 ? a1 - b1 : a2 - b2;
}

interface ProgressEvent {
  bytes: number;
  total: number;
}

interface UpdateContextType {
  isOutdated: boolean;
  latestVersion: string | null;
  downloadProgress: number | null; // 0-100, null = not downloading
  downloadReady: boolean;
  error: string | null;
  startDownload: () => Promise<void>;
  installUpdate: () => Promise<void>;
  setOutdated: (version: string) => void;
}


const UpdateContext = createContext<UpdateContextType | undefined>(undefined);

export const UpdateProvider = ({ children }: { children: ReactNode }) => {
  const [latestVersion, setLatestVersion] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<number | null>(null);
  const [downloadReady, setDownloadReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const setOutdated = (version: string) => {
    setLatestVersion(version);
    // Re-check depuis le DHT: ne remplace que si la version DHT est plus récente
    invoke<string | null>('check_update')
      .then(result => {
        if (result && cmpVersion(result, version) > 0) setLatestVersion(result);
      })
      .catch(() => {});
  };


  // Vérifie la mise à jour au démarrage puis toutes les heures
  useEffect(() => {
    const check = async () => {
      try {
        const result = await invoke<string | null>('check_update');
        if (result) setLatestVersion(result);
      } catch {
        // Silencieux - pas de connexion DHT au démarrage
      }
    };

    check();
    const interval = setInterval(check, 10 * 60 * 1000);
    return () => clearInterval(interval);
  }, []);

  const startDownload = async () => {
    setError(null);
    setDownloadProgress(0);

    const unlisten = await listen<ProgressEvent>('update-progress', ({ payload }) => {
      if (payload.total > 0) {
        setDownloadProgress(Math.round((payload.bytes / payload.total) * 100));
      }
    });

    try {
      await invoke('download_update');
      setDownloadReady(true);
      setDownloadProgress(100);
    } catch (e) {
      setError(String(e));
      setDownloadProgress(null);
    } finally {
      unlisten();
    }
  };

  const installUpdate = async () => {
    if (!downloadReady) return;
    setError(null);
    try {
      await invoke('apply_update');
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <UpdateContext.Provider value={{
      isOutdated: latestVersion !== null,
      latestVersion,
      downloadProgress,
      downloadReady,
      error,
      startDownload,
      installUpdate,
      setOutdated,
    }}>
      {children}
    </UpdateContext.Provider>
  );
};

export const useUpdate = () => {
  const context = useContext(UpdateContext);
  if (!context) throw new Error('useUpdate must be used within UpdateProvider');
  return context;
};
