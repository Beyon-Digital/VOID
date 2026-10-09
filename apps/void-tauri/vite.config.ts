import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Tauri dev expects a fixed port + no polling on Linux.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5175,
    strictPort: true,
    host: false,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: 'es2022',
    minify: 'esbuild',
    sourcemap: true,
  },
});
