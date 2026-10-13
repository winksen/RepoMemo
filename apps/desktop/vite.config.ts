import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    strictPort: true,
    port: 3021,
  },
  // Only these variables are compiled into the bundle. Never the bare
  // `TAURI_` prefix: it would also inline TAURI_SIGNING_PRIVATE_KEY (and its
  // password) during a release build, shipping the updater signing key.
  envPrefix: ["VITE_", "TAURI_ENV_"],
});
