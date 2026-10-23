import { defineConfig } from 'vite';

/**
 * Tauri serves the built files from disk, so asset URLs must stay relative and
 * nothing may be inlined. The Chrome target matches WebView2's engine.
 */
export default defineConfig({
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: 'chrome110',
    outDir: 'dist',
    emptyOutDir: true,
    assetsInlineLimit: 0,
    sourcemap: false,
    chunkSizeWarningLimit: 1200,
  },
  envPrefix: ['VITE_', 'TAURI_'],
});
