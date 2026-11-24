// The build of the page the browser tests load over a fake owner (ts-0179-ke). Its entry is `browser/fake/index.html`,
// which the app's build (`vite.config.ts`, `index.html`) never reads, so the fake owner is in no app bundle. The root
// stays `app/`, so Tailwind finds the classes of `src/`.
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': new URL('./src', import.meta.url).pathname } },
  build: { rollupOptions: { input: 'browser/fake/index.html' } },
})
