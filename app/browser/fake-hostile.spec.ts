// Hostile text in Chrome over a fake owner (ts-0183-ee; plan 0.9, "The webview shows untrusted text"; R5). The page
// runs under the built app's content security policy, which the fake-owner site sends. A task's body and an agent's
// reason are the hostile text (`test/support/hostile.ts`): it must show as text and load, run and open nothing. What
// the page asked the browser to fetch is read from Playwright's request log, and what the policy refused from the
// page's own `securitypolicyviolation` events and the console.
import { type Page } from '@playwright/test'
import { ELSEWHERE, PWNED } from '../test/support/hostile.ts'
import { expect, test } from './support/fixtures.ts'

/** What a page did, collected from the moment it is made. */
interface Watched {
  readonly requests: string[]
  /** The requests the browser refused or could not make, by address, with why. */
  readonly failed: Map<string, string>
  readonly dialogs: string[]
  readonly policyErrors: string[]
}

/** Listens before the page loads. A dialog is dismissed so that the test cannot hang on one. */
function watch(into: Watched) {
  return (page: Page) => {
    page.on('request', (request) => into.requests.push(request.url()))
    page.on('requestfailed', (request) => into.failed.set(request.url(), request.failure()?.errorText ?? ''))
    page.on('dialog', (dialog) => {
      into.dialogs.push(dialog.message())
      void dialog.dismiss()
    })
    page.on('console', (message) => {
      if (/Content Security Policy/i.test(message.text())) into.policyErrors.push(message.text())
    })
  }
}

/** Opens the hostile world and shows its two places of hostile text: the run's prompt and the permission request. */
async function openHostile(openFake: (world: string, watch: (page: Page) => void) => Promise<Page>) {
  const seen: Watched = { requests: [], failed: new Map(), dialogs: [], policyErrors: [] }
  const page = await openFake('hostile-text', watch(seen))
  await expect(page.getByRole('region', { name: 'Field' })).toBeVisible()
  await page.keyboard.press('3')
  await page.getByRole('button', { name: 'Open the Loop page' }).click()
  await expect(page.getByRole('heading', { name: 'Steps' }).first()).toBeVisible()
  return { page, seen }
}

/** Whether the page has the flag the payloads would set. */
function flag(page: Page): Promise<unknown> {
  return page.evaluate((name) => (window as unknown as Record<string, unknown>)[name], PWNED)
}

test('the hostile body shows as text, and no request leaves the page, no dialog opens and no payload runs', async ({
  openFake,
}) => {
  const { page, seen } = await openHostile(openFake)
  const origin = new URL(page.url()).origin

  // Raw HTML is text, and a refused link is text with the reason.
  await expect(page.getByText('<script>window.__pwned', { exact: false }).first()).toBeVisible()
  await expect(page.getByText('(link not opened: javascript: links are never opened)').first()).toBeVisible()
  await expect(page.getByText('(link not opened: file: links are never opened)').first()).toBeVisible()
  await expect(page.getByText('(link not opened: data: links are never opened)').first()).toBeVisible()
  await expect(
    page.getByText(`[image not loaded: tracking pixel, from ${ELSEWHERE}/pixel.png]`).first(),
  ).toBeVisible()

  // The page has no script, image, link, or frame or form in the text, that the text made.
  const made = await page.evaluate(() => ({
    scripts: [...document.scripts].filter((script) => script.textContent?.includes('__pwned')).length,
    images: document.images.length,
    frames: document.querySelectorAll('.md iframe, .md object, .md embed, .md form').length,
    anchors: document.querySelectorAll('a[href]').length,
  }))
  expect(made).toEqual({ scripts: 0, images: 0, frames: 0, anchors: 0 })

  // Nothing was fetched from beyond the page's own origin, nothing was asked of the discard port, and no dialog opened.
  const elsewhere = seen.requests.filter((url) => !url.startsWith(origin) && !url.startsWith('data:'))
  expect(elsewhere).toEqual([])
  expect(seen.requests.filter((url) => url.includes(':9/'))).toEqual([])
  expect(seen.dialogs).toEqual([])
  expect(await flag(page)).toBeUndefined()

  // Normal use is not refused by the policy either: it would be if the page needed inline code or a remote origin.
  expect(seen.policyErrors).toEqual([])
})

test('an image the test adds is refused by the policy, and so is inline script', async ({ openFake }) => {
  const { page, seen } = await openHostile(openFake)
  const violations = await page.evaluate(
    (source) =>
      new Promise<string[]>((resolve) => {
        const directives: string[] = []
        document.addEventListener('securitypolicyviolation', (event) => {
          directives.push(event.effectiveDirective)
          if (directives.length === 2) resolve(directives)
        })
        const image = document.createElement('img')
        image.src = `${source}/added.png`
        document.body.append(image)
        const script = document.createElement('script')
        script.textContent = `window.__pwned = 'inline'`
        document.body.append(script)
      }),
    ELSEWHERE,
  )
  expect(violations.sort()).toEqual(['img-src', 'script-src-elem'])
  expect(await flag(page)).toBeUndefined()
  // The browser logs the request it was asked for, and then refuses it: Chrome says it failed for `csp`, not for lack
  // of a listener on the port.
  await expect.poll(() => seen.failed.get(`${ELSEWHERE}/added.png`)).toBe('csp')
})

test('a kept link asks first: Cancel opens nothing, and Open asks the shell for the one address', async ({
  openFake,
}) => {
  const { page } = await openHostile(openFake)
  const opened = () => page.evaluate(() => window.collideFake.opened())
  const asking = page.getByRole('dialog', { name: 'Open this link in your browser?' })
  const url = 'https://example.com/docs?page=1&lang=en'

  // A refused link is no button, and a kept one has no address to follow.
  await expect(page.getByRole('button', { name: 'script link' })).toHaveCount(0)
  await expect(page.getByRole('link')).toHaveCount(0)
  await expect(asking).toHaveCount(0)

  await page.getByRole('button', { name: 'the docs' }).first().click()
  await expect(asking).toBeVisible()
  await expect(asking).toContainText(url)
  await expect(asking).toContainText('example.com')
  expect(await opened()).toEqual([])

  await asking.getByRole('button', { name: 'Cancel' }).click()
  await expect(asking).toHaveCount(0)
  expect(await opened()).toEqual([])

  // Escape is Cancel too.
  await page.getByRole('button', { name: 'the docs' }).first().click()
  await expect(asking).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(asking).toHaveCount(0)
  expect(await opened()).toEqual([])

  await page.getByRole('button', { name: 'the docs' }).first().click()
  await asking.getByRole('button', { name: 'Open in browser' }).click()
  await expect(asking).toHaveCount(0)
  expect(await opened()).toEqual([url])
})

test('each way out of the link window gives the focus back to the link', async ({ openFake }) => {
  const { page } = await openHostile(openFake)
  const asking = page.getByRole('dialog', { name: 'Open this link in your browser?' })
  const link = page.getByRole('button', { name: 'the docs' }).first()
  for (const way of ['Cancel', 'Escape', 'Open in browser']) {
    await link.click()
    await expect(asking).toBeVisible()
    if (way === 'Escape') await page.keyboard.press('Escape')
    else await asking.getByRole('button', { name: way }).click()
    await expect(asking).toHaveCount(0)
    await expect(link, `after ${way}`).toBeFocused()
  }
})

test('a mail link asks first too, and names no host', async ({ openFake }) => {
  const { page } = await openHostile(openFake)
  await page.getByRole('button', { name: 'write to us' }).first().click()
  const asking = page.getByRole('dialog', { name: 'Open this link in your browser?' })
  await expect(asking).toContainText('mailto:team@example.com')
  await expect(asking).not.toContainText('It leads to')
  await asking.getByRole('button', { name: 'Cancel' }).click()
  expect(await page.evaluate(() => window.collideFake.opened())).toEqual([])
})
