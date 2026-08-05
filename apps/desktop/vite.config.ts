import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri 2 expects a fixed dev server port and no auto-clearing of the screen.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    target: "es2021",
    outDir: "dist",
    chunkSizeWarningLimit: 1500,
    rollupOptions: {
      output: {
        manualChunks: {
          // 返工修复：G6/重模块真正拆包，减小主 chunk
          g6: ["@antv/g6"],
          react: ["react", "react-dom"],
        },
      },
    },
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
  },
});
