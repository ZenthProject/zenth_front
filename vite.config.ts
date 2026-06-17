import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

const host = process.env.TAURI_DEV_HOST;
const port = process.env.VITE_PORT ? parseInt(process.env.VITE_PORT) : 1420;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react({ include: /\.(tsx|ts|jsx|js)$/ }), tailwindcss()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  build: {
    // Désactive les <link rel="modulepreload" crossorigin> générés par Vite.
    // Sur Windows, WebView2 tente de charger ces chunks en mode CORS ce qui
    // peut échouer silencieusement avec le protocole custom https://tauri.localhost/
    modulePreload: false,
    rollupOptions: {
      output: {
        // Seuls les chunks sans dépendance d'init sur React sont isolés ici.
        // Séparer react/react-dom/radix/i18n/etc. dans des chunks distincts
        // a causé un crash "React.Children undefined" : avec modulePreload
        // désactivé (cf. plus haut), rien ne garantit que vendor-react
        // s'exécute avant les chunks qui en dépendent au chargement du module.
        manualChunks: (id) => {
          if (!id.includes('node_modules')) return undefined;
          if (id.includes('emoji-picker-react'))                   return 'vendor-emoji';
          if (id.includes('qr-code-styling') || id.includes('qrcode')) return 'vendor-qr';
          return undefined;
        },
      },
    },
  },
}));
