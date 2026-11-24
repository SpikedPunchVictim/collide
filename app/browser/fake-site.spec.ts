// The server of the fake-owner page answers as the bridge's page route does (ts-0179-ke): only on loopback, to `GET`,
// for a plain name below the built folder, and always with `Cache-Control: no-store`.
import { request } from 'node:http'
import { expect, test } from './support/fixtures.ts'

interface Answer {
  readonly status: number
  readonly headers: Record<string, string | string[] | undefined>
}

/** What the server answers to a request as written: `Host` and the rest are the test's own. */
function ask(url: string, path: string, method: string, headers: Record<string, string> = {}): Promise<Answer> {
  return new Promise((resolve, reject) => {
    const { port } = new URL(url)
    const send = request(
      { host: '127.0.0.1', port: Number(port), method, path, headers: { host: `127.0.0.1:${port}`, ...headers } },
      (answer) => {
        answer.resume()
        answer.on('end', () => resolve({ status: answer.statusCode ?? 0, headers: answer.headers }))
      },
    )
    send.on('error', reject)
    send.end()
  })
}

test('the page is served with the headers of the bridge', async ({ fakeSite }) => {
  const page = await ask(fakeSite.url, '/', 'GET')
  expect(page.status).toBe(200)
  expect(page.headers['content-type']).toBe('text/html; charset=utf-8')
  expect(page.headers['cache-control']).toBe('no-store')
  expect(page.headers.connection).toBe('close')
})

test('the server refuses a foreign Host or Origin, a write, and a path outside the page', async ({ fakeSite }) => {
  const port = new URL(fakeSite.url).port
  expect((await ask(fakeSite.url, '/', 'GET', { host: 'evil.example' })).status).toBe(403)
  expect((await ask(fakeSite.url, '/', 'GET', { host: `localhost.evil.example:${port}` })).status).toBe(403)
  expect((await ask(fakeSite.url, '/', 'GET', { origin: 'http://evil.example' })).status).toBe(403)
  expect((await ask(fakeSite.url, '/', 'POST')).status).toBe(405)
  expect((await ask(fakeSite.url, '/../x', 'GET')).status).toBe(404)
  expect((await ask(fakeSite.url, '/assets/%2e%2e/x', 'GET')).status).toBe(404)
  expect((await ask(fakeSite.url, '/missing.js', 'GET')).status).toBe(404)
})
