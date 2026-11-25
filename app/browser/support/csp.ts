// The built app's content security policy, read from the file that sets it (`src-tauri/tauri.conf.json`,
// `app.security.csp`) and written as a header the way `collide_app::csp::built` writes it (ts-0183-ee). The fake-owner
// page is served with it, and a bridge spec compares the bridge's header to it.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

const CONFIG = join(import.meta.dirname, '..', '..', 'src-tauri', 'tauri.conf.json')

/** The policy as a header value: `name source source; name source`. */
export function builtCsp(): string {
  const config = JSON.parse(readFileSync(CONFIG, 'utf8')) as { app: { security: { csp: Record<string, string[]> } } }
  return Object.entries(config.app.security.csp)
    .map(([name, sources]) => `${name} ${sources.join(' ')}`)
    .join('; ')
}

/** The policy the test bridge sends: the built one, with `'self'` first in `connect-src` for its HTTP calls. */
export function bridgePageCsp(): string {
  return builtCsp().replace('connect-src ', "connect-src 'self' ")
}
