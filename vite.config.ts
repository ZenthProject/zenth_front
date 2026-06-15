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
        manualChunks: (id) => {
          if (!id.includes('node_modules')) return undefined;
          if (id.includes('emoji-picker-react'))                   return 'vendor-emoji';
          if (id.includes('qr-code-styling') || id.includes('qrcode')) return 'vendor-qr';
          if (id.includes('lucide-react'))                         return 'vendor-icons';
          if (id.includes('@radix-ui'))                            return 'vendor-radix';
          if (id.includes('i18next'))                              return 'vendor-i18n';
          if (id.includes('@tauri-apps'))                          return 'vendor-tauri';
          if (id.includes('react-dom') || id.includes('react-router') || id.includes('/react/')) return 'vendor-react';
          return 'vendor';
        },
      },
    },
  },
}));
