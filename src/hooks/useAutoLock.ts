import { useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { useAuth } from "@/contexts/AuthContext";

const ACTIVITY_EVENTS = ["mousemove", "mousedown", "keydown", "touchstart", "scroll"] as const;
const CHECK_INTERVAL_MS = 15_000; // vérification toutes les 15s

export function useAutoLock() {
  const { lock, isAuthenticated } = useAuth();
  const navigate = useNavigate();
  const lastActivityRef = useRef(Date.now());
  const lockRef = useRef(lock);
  lockRef.current = lock;

  useEffect(() => {
    if (!isAuthenticated) return;

    const updateActivity = () => { lastActivityRef.current = Date.now(); };

    const checkInactivity = () => {
      const enabled = (localStorage.getItem("zenth_auto_lock_enabled") ?? "true") === "true";
      if (!enabled) return;

      const minutes = parseInt(localStorage.getItem("zenth_auto_lock_timeout") ?? "5", 10);
      if (isNaN(minutes) || minutes <= 0) return;

      if (Date.now() - lastActivityRef.current >= minutes * 60_000) {
        lockRef.current().then(() => navigate("/login"));
      }
    };

    ACTIVITY_EVENTS.forEach(e => window.addEventListener(e, updateActivity, { passive: true }));
    const interval = setInterval(checkInactivity, CHECK_INTERVAL_MS);

    return () => {
      ACTIVITY_EVENTS.forEach(e => window.removeEventListener(e, updateActivity));
      clearInterval(interval);
    };
  }, [isAuthenticated, navigate]);
}
