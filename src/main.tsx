import React, { Component, ReactNode } from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router";
import "./App.css";
import "./lib/i18n";
import { router } from "./lib/routes";
import { AuthProvider } from "./contexts/AuthContext";
import { WebSocketProvider } from "./contexts/WebSocketContext";
import { ThemeProvider } from "./lib/theme";
import { UpdateProvider } from "./contexts/UpdateContext";

class AppErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null };
  static getDerivedStateFromError(error: Error) { return { error }; }
  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: 32, fontFamily: "monospace", color: "#f87171", background: "#0a0a0a", minHeight: "100vh" }}>
          <h2>Zenth: erreur de démarrage</h2>
          <pre style={{ whiteSpace: "pre-wrap", fontSize: 12 }}>
            {(this.state.error as Error).message}
            {"\n"}
            {(this.state.error as Error).stack}
          </pre>
        </div>
      );
    }
    return this.props.children;
  }
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppErrorBoundary>
      <React.Suspense fallback={<div style={{ background: "#0a0a0a", minHeight: "100vh" }} />}>
        <ThemeProvider>
          <UpdateProvider>
            <AuthProvider>
              <WebSocketProvider>
                <RouterProvider router={router} />
              </WebSocketProvider>
            </AuthProvider>
          </UpdateProvider>
        </ThemeProvider>
      </React.Suspense>
    </AppErrorBoundary>
  </React.StrictMode>
);
