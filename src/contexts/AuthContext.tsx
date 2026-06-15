import { createContext, useContext, useState, useCallback, useEffect, useRef, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface AuthContextType {
  isAuthenticated: boolean;
  isLoading: boolean;
  username: string | null;
  sessionToken: string | null;
  login: (username: string, sessionToken: string, password?: string) => void;
  logout: () => Promise<void>;
  lock: () => Promise<void>;
  restoreFromPersist: () => Promise<boolean>;
}

const AUTH_STORAGE_KEY = 'zenth_auth';
export const REMEMBER_USERNAME_KEY = 'zenth_remember_username';
const SESSION_PW_KEY = 'zenth_session_pw';

const AuthContext = createContext<AuthContextType | undefined>(undefined);

// Erreurs considérées comme permanentes → on efface les credentials
const PERMANENT_AUTH_ERRORS = ["INVALID_CREDENTIALS", "ACCOUNT_NOT_FOUND", "INVALID_PASSWORD", "USER_NOT_FOUND"];

function isPermanentAuthError(err: unknown): boolean {
  const msg = err instanceof Error ? err.message : String(err);
  return PERMANENT_AUTH_ERRORS.some(e => msg.toUpperCase().includes(e));
}

async function _tryAutoLogin(
  setUsername: (u: string | null) => void,
  setSessionToken: (t: string | null) => void,
  setIsAuthenticated: (v: boolean) => void,
): Promise<boolean> {
  let u: string | null = null;
  let p: string | null = null;

  try {
    const cred = await invoke<{ username: string; password: string } | null>('retrieve_credential');
    if (cred) { u = cred.username; p = cred.password; }
  } catch {}

  if (!u || !p) return false;

  try {
    const token = await invoke<string>('login', { username: u, password: p });
    const payload = JSON.stringify({ username: u, sessionToken: token });
    setUsername(u);
    setSessionToken(token);
    setIsAuthenticated(true);
    sessionStorage.setItem(AUTH_STORAGE_KEY, payload);
    localStorage.setItem(REMEMBER_USERNAME_KEY, u);
    return true;
  } catch (err) {
    // Erreur permanente (mauvais mdp, compte supprimé) → purge les credentials
    // Erreur transitoire (serveur indisponible, réseau) → on garde les credentials pour le prochain démarrage
    if (isPermanentAuthError(err)) {
      invoke('delete_credential').catch(() => {});
      localStorage.removeItem(AUTH_STORAGE_KEY);
    }
    return false;
  }
}

export const AuthProvider = ({ children }: { children: ReactNode }) => {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [username, setUsername] = useState<string | null>(null);
  const [sessionToken, setSessionToken] = useState<string | null>(null);

  // Restaure la session au démarrage.
  //
  // Cas 1 - sessionStorage (même processus, rechargement WebView) :
  //   Le token Rust est encore valide → restauration directe.
  //
  // Cas 2 - localStorage + persist_session (restart app) :
  //   Le backend Rust repart de zéro → check_session échoue systématiquement.
  //   On utilise les credentials stockés pour appeler login() automatiquement.
  //
  // Cas 3 - credentials corrompus ou expirés → on redirige vers le login
  //   avec le username pré-rempli.
  useEffect(() => {
    const restore = async () => {
      try {
        const persist = localStorage.getItem("zenth_persist_session") === "true";
        const fromSession = sessionStorage.getItem(AUTH_STORAGE_KEY);

        // Cas 1 : même processus - token encore valide côté Rust
        if (fromSession) {
          const { username: u, sessionToken: t } = JSON.parse(fromSession);
          if (u && t) {
            try {
              await invoke('check_session', { sessionToken: t });
              setUsername(u);
              setSessionToken(t);
              setIsAuthenticated(true);
              localStorage.setItem(REMEMBER_USERNAME_KEY, u);
              return;
            } catch {
              // Token expiré même dans le même processus (timeout Rust)
              sessionStorage.removeItem(AUTH_STORAGE_KEY);
            }
          }
        }

        // Cas 2 : restart avec persist_session → auto-login avec credentials stockés
        if (persist) {
          await _tryAutoLogin(setUsername, setSessionToken, setIsAuthenticated);
        }
      } catch {
        sessionStorage.removeItem(AUTH_STORAGE_KEY);
        invoke('delete_credential').catch(() => {});
      } finally {
        setIsLoading(false);
      }
    };
    restore();
  }, []);

  const login = useCallback((user: string, token: string, password?: string) => {
    const payload = JSON.stringify({ username: user, sessionToken: token });
    setUsername(user);
    setSessionToken(token);
    setIsAuthenticated(true);
    sessionStorage.setItem(AUTH_STORAGE_KEY, payload);
    localStorage.setItem(REMEMBER_USERNAME_KEY, user);
    // Garde le mdp en sessionStorage pour pouvoir le stocker si persist_session
    // est activé après coup depuis les paramètres (sessionStorage est vidé à la fermeture)
    if (password) sessionStorage.setItem(SESSION_PW_KEY, password);

    if (localStorage.getItem("zenth_persist_session") === "true" && password) {
      void invoke('store_credential', { username: user, password });
    }
  }, []);

  // Relay polling global - fonctionne sur toutes les pages
  const relayTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  useEffect(() => {
    if (!isAuthenticated || !sessionToken) return;

    const poll = async () => {
      try {
        const count: number = await invoke("relay_pull_messages", { sessionToken });
        if (count > 0) {
          window.dispatchEvent(new CustomEvent("relay:update", { detail: { count } }));
        }
      } catch {
        // Pas de device jumelé ou DHT indisponible - silencieux
      }
    };

    poll();
    relayTimerRef.current = setInterval(poll, 10000);
    return () => {
      if (relayTimerRef.current) clearInterval(relayTimerRef.current);
    };
  }, [isAuthenticated, sessionToken]);

  const logout = useCallback(async () => {
    if (sessionToken) {
      try {
        await invoke('logout', { sessionToken });
      } catch (e) {
        console.error('[logout] Backend cleanup failed:', e);
      }
    }
    setUsername(null);
    setSessionToken(null);
    setIsAuthenticated(false);
    sessionStorage.removeItem(AUTH_STORAGE_KEY);
    sessionStorage.removeItem(SESSION_PW_KEY);
    await invoke('delete_credential').catch(() => {});
    // On garde REMEMBER_USERNAME_KEY pour pré-remplir le login
  }, [sessionToken]);

  // Tente une reconnexion silencieuse depuis les credentials persistés.
  // Retourne true si la reconnexion a réussi.
  const restoreFromPersist = useCallback(async (): Promise<boolean> => {
    if (localStorage.getItem("zenth_persist_session") !== "true") return false;
    return _tryAutoLogin(setUsername, setSessionToken, setIsAuthenticated);
  }, []);

  // Verrouille l'interface sans supprimer les credentials de persistance.
  // Utilisé par l'auto-lock : l'utilisateur doit se ré-authentifier,
  // mais les credentials sauvegardés dans le Keystore restent intacts
  // pour le prochain démarrage complet de l'application.
  const lock = useCallback(async () => {
    if (sessionToken) {
      try {
        await invoke('logout', { sessionToken });
      } catch {}
    }
    setUsername(null);
    setSessionToken(null);
    setIsAuthenticated(false);
    sessionStorage.removeItem(AUTH_STORAGE_KEY);
    sessionStorage.removeItem(SESSION_PW_KEY);
    // Les credentials de persistance (Keystore) ne sont PAS supprimés
  }, [sessionToken]);

  return (
    <AuthContext.Provider value={{
      isAuthenticated,
      isLoading,
      username,
      sessionToken,
      login,
      logout,
      lock,
      restoreFromPersist,
    }}>
      {children}
    </AuthContext.Provider>
  );
};

export const useAuth = () => {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error('useAuth must be used within AuthProvider');
  }
  return context;
};
