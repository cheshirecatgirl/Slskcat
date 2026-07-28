import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri drives this dev server; the fixed port and host let the webview find
// it, and the strict flag makes a port clash fail loudly instead of silently
// serving on a port Tauri is not pointed at.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  // No sourcemap in the shipped bundle: it is 6x the size of the code it maps.
  build: { target: "es2022", sourcemap: false },
});
