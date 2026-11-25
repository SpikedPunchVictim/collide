// A small `node:http` server for the page built over a fake owner (ts-0179-ke). It stands where the bridge stands for
// the other page, with what the bridge's page route does (`collide-bridge`, `http.rs`): loopback only, a foreign `Host`
// or `Origin` is refused, `GET` only, a path that is not a plain name below the folder is not found, and every answer
// says `Cache-Control: no-store` and `Connection: close`. A file is sent with the built app's CSP (ts-0183-ee). It has
// no `/rpc`: the fake owner is in the page.
import { readFile } from 'node:fs/promises'
import { type IncomingMessage, type ServerResponse, createServer } from 'node:http'
import { extname, join } from 'node:path'
import { builtCsp } from './csp.ts'

/** The page, as the built folder holds it: vite keeps the path of the html entry. */
const PAGE = 'browser/fake/index.html'

const TYPES: Readonly<Record<string, string>> = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.woff2': 'font/woff2',
}

export interface FakeSite {
  /** The address of the page, `http://127.0.0.1:<port>`. */
  readonly url: string
  /** Stops listening and drops what is open. Finishes when the port is free. */
  close(): Promise<void>
}

/** The file below `dir` that `path` names, or null: `/` is the page; a dot-dot, an escape or a backslash is none. */
export function fileFor(dir: string, path: string): string | null {
  const relative = path === '/' ? PAGE : path.slice(1)
  if (!path.startsWith('/') || relative.includes('%') || relative.includes('\\')) return null
  const parts = relative.split('/')
  if (parts.some((part) => part === '' || part === '.' || part === '..')) return null
  return join(dir, ...parts)
}

function answer(
  response: ServerResponse,
  status: number,
  type: string,
  body: Buffer | string,
  extra: Record<string, string> = {},
): void {
  response.writeHead(status, {
    'Content-Type': type,
    'Content-Length': Buffer.byteLength(body),
    'Cache-Control': 'no-store',
    Connection: 'close',
    ...extra,
  })
  response.end(body)
}

function refusal(response: ServerResponse, status: number, words: string): void {
  answer(response, status, 'text/plain; charset=utf-8', words)
}

export async function serveFakePage(dir: string): Promise<FakeSite> {
  const server = createServer((request, response) => void handle(dir, port(), request, response))
  const port = () => (server.address() as { port: number }).port
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject)
    server.listen(0, '127.0.0.1', resolve)
  })
  return {
    url: `http://127.0.0.1:${port()}`,
    close: () =>
      new Promise<void>((resolve) => {
        server.close(() => resolve())
        server.closeAllConnections()
      }),
  }
}

async function handle(dir: string, port: number, request: IncomingMessage, response: ServerResponse): Promise<void> {
  const host = request.headers.host ?? ''
  if (host !== `127.0.0.1:${port}` && host !== `localhost:${port}`) return refusal(response, 403, 'Forbidden')
  const origin = request.headers.origin
  if (origin !== undefined && origin !== `http://${host}`) return refusal(response, 403, 'Forbidden')
  if (request.method !== 'GET') return refusal(response, 405, 'Method Not Allowed')
  const file = fileFor(dir, (request.url ?? '').split('?')[0] ?? '')
  if (file === null) return refusal(response, 404, 'Not Found')
  try {
    const type = TYPES[extname(file)] ?? 'application/octet-stream'
    answer(response, 200, type, await readFile(file), { 'Content-Security-Policy': builtCsp() })
  } catch {
    refusal(response, 404, 'Not Found')
  }
}
