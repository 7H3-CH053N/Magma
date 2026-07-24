import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { readFileSync } from "node:fs";

const pkg = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf-8"));
// A date-stamped build id (YYYYMMDD) so the About panel shows a real build.
const now = new Date();
const buildId =
  `${now.getUTCFullYear()}` +
  `${String(now.getUTCMonth() + 1).padStart(2, "0")}` +
  `${String(now.getUTCDate()).padStart(2, "0")}`;

// Tauri expects a fixed dev-server port and ignores the src-tauri folder.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
    __BUILD_ID__: JSON.stringify(process.env.BUILD_NUMBER ?? buildId),
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "esnext",
    outDir: "dist",
  },
});
