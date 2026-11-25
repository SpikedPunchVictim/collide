// Which addresses in a task's text may open (ts-0183-ee; plan 0.9, R5): the web and a mail address, and nothing else.
// The address is read as a browser reads it, so `JavaScript:`, a tab or a newline inside the scheme and a leading space
// do not get past it. The shell checks again before it opens anything (`shell.open_external`).

/** The schemes of a link that may open. */
const OPENABLE = new Set(['http:', 'https:', 'mailto:'])

function parse(raw: string | undefined): URL | null {
  if (raw === undefined) return null
  try {
    return new URL(raw)
  } catch {
    // No scheme (a path, `//host`, a bare name) or no host: nothing to open.
    return null
  }
}

/** The address as the shell would be given it, or null when the link may not open. */
export function openableLink(raw: string | undefined): string | null {
  const url = parse(raw)
  if (url === null || !OPENABLE.has(url.protocol)) return null
  const named = url.protocol === 'mailto:' ? url.pathname : url.hostname
  return named === '' ? null : url.href
}

/** The scheme a refused link is named by (`javascript:`), or null when it has none. */
export function schemeOf(raw: string | undefined): string | null {
  return parse(raw)?.protocol ?? null
}
