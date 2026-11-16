// The bridge answers only on loopback and only to its own page (ts-0082-fp). It refuses a foreign `Host` (DNS
// rebinding), a foreign `Origin`, a call without the run's token and a body that is not JSON, as the S6 record's
// endpoint did (`.collide/planning/spikes/sp-0009-bb-interface-prototype/record.md`, the MCP endpoint's checks).
import { request } from 'node:http'
import { expect, test } from './support/fixtures.ts'
import { firstPhase } from './support/recordings.ts'
import { type World } from './support/world.ts'

interface Call {
  readonly method?: string
  readonly path?: string
  readonly headers?: Record<string, string>
  readonly body?: string
}

/** The status the bridge answers with, for a request sent as written: `Host` and `Origin` are the test's own. */
function statusOf(world: World, call: Call): Promise<number> {
  return new Promise((resolve, reject) => {
    const port = Number(new URL(world.url).port)
    const send = request(
      { host: '127.0.0.1', port, method: call.method ?? 'POST', path: call.path ?? '/rpc', headers: call.headers },
      (answer) => {
        answer.resume()
        answer.on('end', () => resolve(answer.statusCode ?? 0))
      },
    )
    send.on('error', reject)
    send.end(call.body ?? '')
  })
}

function allowed(world: World): Record<string, string> {
  return {
    host: new URL(world.url).host,
    authorization: `Bearer ${world.token}`,
    'content-type': 'application/json',
  }
}

const LIST = JSON.stringify({ method: 'board.list', params: {} })

test('the bridge refuses a foreign Host, a foreign Origin and a call without the token', async ({ startWorld }) => {
  const world = await startWorld({ first: firstPhase([]) })
  const headers = allowed(world)

  // The call as the page makes it works.
  expect(await statusOf(world, { headers, body: LIST })).toBe(200)

  // A foreign Host, whether the call or the page.
  const foreign = { ...headers, host: 'evil.example' }
  expect(await statusOf(world, { headers: foreign, body: LIST })).toBe(403)
  expect(await statusOf(world, { method: 'GET', path: '/', headers: { host: 'evil.example' } })).toBe(403)
  const lookalike = { ...headers, host: `localhost.evil.example:${new URL(world.url).port}` }
  expect(await statusOf(world, { headers: lookalike, body: LIST })).toBe(403)

  // A foreign Origin, and one that only names another port of the loopback.
  expect(await statusOf(world, { headers: { ...headers, origin: 'http://evil.example' }, body: LIST })).toBe(403)
  expect(await statusOf(world, { headers: { ...headers, origin: 'http://127.0.0.1:1' }, body: LIST })).toBe(403)

  // No token, a wrong one, and one that is not a bearer.
  const { authorization: _, ...bare } = headers
  expect(await statusOf(world, { headers: bare, body: LIST })).toBe(401)
  expect(await statusOf(world, { headers: { ...headers, authorization: 'Bearer nope' }, body: LIST })).toBe(401)
  expect(await statusOf(world, { headers: { ...headers, authorization: world.token }, body: LIST })).toBe(401)
  expect(await statusOf(world, { method: 'GET', path: '/events', headers: bare })).toBe(401)
  expect(await statusOf(world, { path: '/shutdown', headers: bare, body: '{}' })).toBe(401)

  // A body that is not JSON, so a browser has to preflight, and a preflight is never answered.
  expect(await statusOf(world, { headers: { ...headers, 'content-type': 'text/plain' }, body: LIST })).toBe(415)
  expect(await statusOf(world, { method: 'OPTIONS', headers: { ...headers, origin: `http://evil.example` } })).toBe(403)

  // Nothing the refused calls asked for was done.
  expect((await world.rpc('board.list')).boards).toEqual([])
})

test('a page of another origin cannot call the bridge', async ({ startWorld, openPage }) => {
  const world = await startWorld({ first: firstPhase([]) })
  const page = await openPage(world)
  // A page that Chrome takes for another origin, which the test serves itself so that nothing leaves the machine.
  await page.route('http://other.test/', (route) =>
    route.fulfill({ contentType: 'text/html', body: '<!doctype html><title>other</title>' }),
  )
  await page.goto('http://other.test/')
  const reached = await page.evaluate(
    async ({ url, token }) => {
      try {
        const answer = await fetch(`${url}/rpc`, {
          method: 'POST',
          headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
          body: JSON.stringify({ method: 'board.list', params: {} }),
        })
        return `answered ${answer.status}`
      } catch (failure) {
        return `blocked: ${String(failure)}`
      }
    },
    { url: world.url, token: world.token },
  )
  expect(reached).toMatch(/^blocked/)
})
