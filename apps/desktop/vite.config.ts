import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

// Tauri serves the built files itself; in development it loads this server (src-tauri/tauri.conf.json).
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true, host: "127.0.0.1" },
  // Old phones too: the Android test device's System WebView is Chrome 67 (src/polyfills.ts fills the gaps).
  build: { target: ["chrome67", "safari12"], outDir: "dist", emptyOutDir: true },
});
