import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";
import pkg from "./package.json";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes("/node_modules/@tanstack/")) return "tanstack";
          if (id.includes("/node_modules/@radix-ui/")) return "radix";
        },
      },
    },
  },
});
