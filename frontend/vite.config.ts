import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port and a relative base for the bundled assets.
export default defineConfig({
  plugins: [react()],
  base: "./",
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { outDir: "dist", target: "es2021", sourcemap: false },
  resolve: {
    alias: {
      // SBAI-5433: tinyusdz's optional zstd-WASM path needs `fzstd`, which we
      // don't ship — alias it to an explicit-failure stub so the bundler can
      // resolve the dynamic import (see src/content/usd/fzstd-stub.ts).
      fzstd: "/src/content/usd/fzstd-stub.ts",
    },
  },
});
