/// <reference types="vitest/config" />
import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig, type Plugin } from 'vite'
import { collideAgent } from './server/plugin.ts'

// Dev-server endpoint: the facilitator panel can POST a session log here and it lands in out/sessions/.
function saveSessions(): Plugin {
  return {
    name: 'save-sessions',
    configureServer(server) {
      server.middlewares.use('/__session', (req, res) => {
        if (req.method !== 'POST') {
          res.statusCode = 405
          res.end()
          return
        }
        let body = ''
        req.on('data', (c) => (body += c))
        req.on('end', () => {
          try {
            const dir = join(process.cwd(), 'out', 'sessions')
            mkdirSync(dir, { recursive: true })
            const name = `session-${new Date().toISOString().replace(/[:.]/g, '-')}.json`
            writeFileSync(join(dir, name), JSON.stringify(JSON.parse(body), null, 2))
            res.setHeader('content-type', 'application/json')
            res.end(JSON.stringify({ saved: `out/sessions/${name}` }))
          } catch (e) {
            res.statusCode = 400
            res.end(String(e))
          }
        })
      })
    },
  }
}

export default defineConfig({
  plugins: [react(), saveSessions(), collideAgent()],
  server: { host: '127.0.0.1' },
  test: { environment: 'jsdom', include: ['test/**/*.test.{ts,tsx}'], testTimeout: 30_000 },
})
