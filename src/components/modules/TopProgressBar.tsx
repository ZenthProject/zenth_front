import { useNavigation } from "react-router-dom";
import { useEffect, useRef, useState } from "react";

export function TopProgressBar() {
  const navigation = useNavigation();
  const [width, setWidth] = useState(0);
  const [visible, setVisible] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const doneRef = useRef(false);

  useEffect(() => {
    if (navigation.state !== "idle") {
      doneRef.current = false;
      setVisible(true);
      setWidth(0);
      // Simule une progression rapide jusqu'à 85%
      timerRef.current = setTimeout(() => setWidth(85), 20);
    } else {
      if (!visible) return;
      doneRef.current = true;
      setWidth(100);
      timerRef.current = setTimeout(() => {
        setVisible(false);
        setWidth(0);
      }, 300);
    }
    return () => { if (timerRef.current) clearTimeout(timerRef.current); };
  }, [navigation.state]);

  if (!visible) return null;

  return (
    <div
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        height: "2px",
        width: `${width}%`,
        background: "linear-gradient(to right, var(--color-primary), var(--color-accent-secondary))",
        transition: width === 100 ? "width 0.15s ease-out" : "width 0.6s ease-in-out",
        zIndex: 9999,
        borderRadius: "0 2px 2px 0",
      }}
    />
  );
}
