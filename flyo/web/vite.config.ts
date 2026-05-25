import { defineConfig } from 'vite';
import preact from '@preact/preset-vite';

// During dev, proxy /api/* to the running flyo backend so we can iterate on UI
// without manually wiring CORS. In production the same binary serves both
// the UI and the API, so no proxy is needed.
export default defineConfig({
  plugins: [preact()],
  server: {
    port: 5173,
    proxy: {
      '/api': 'http://127.0.0.1:9215',
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: 'es2022',
    cssMinify: 'lightningcss',
    minify: 'esbuild',
    sourcemap: false,
    // We embed dist/ into the Rust binary; small chunks are easier to gzip.
    rollupOptions: {
      output: {
        manualChunks: undefined,
      },
    },
  },
});
