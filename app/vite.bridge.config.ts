// The build of the page the browser tests load from the bridge (ts-0082-fp). It has its own entry, `bridge.html`,
// which the app's build (`vite.config.ts`, `index.html`) never reads, so the bridge's client is in no app bundle.
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': new URL('./src', import.meta.url).pathname } },
  build: { rollupOptions: { input: 'bridge.html' } },
})
