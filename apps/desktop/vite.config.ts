import { defineConfig } from "vite";

// Tauri serves the built files itself; in development it loads this server (src-tauri/tauri.conf.json).
export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true, host: "127.0.0.1" },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true },
});
