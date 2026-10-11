import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Vite serves the UI for `tauri dev` (port 5760) and builds `dist/` for the desktop bundle
// and for the browser/e2e build (same bundle; the engine backend is chosen at runtime).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5760, strictPort: true },
  build: { target: 'es2022', sourcemap: false, chunkSizeWarningLimit: 2000 },
  assetsInclude: ['**/*.wasm'],
});
