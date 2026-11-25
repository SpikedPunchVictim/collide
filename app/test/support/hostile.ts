// The hostile body of a task (ts-0183-ee; plan 0.9, "The webview shows untrusted text"): what a person outside the
// repo could write into a task, an issue or an agent's words. Every payload that would run sets `window.__pwned`, so a
// test can see that none did. The addresses are loopback, on a port nothing listens on (the discard port), so a payload
// that did load would find nobody to answer, and nothing in a test reaches another machine.

/** A marker the payloads below would set if they ran. */
export const PWNED = '__pwned'

/** A host that is not the page's own origin and answers nobody. */
export const ELSEWHERE = 'http://127.0.0.1:9'

export const HOSTILE_BODY = [
  '## Steps',
  '',
  `<script>window.${PWNED} = 'script'</script>`,
  '',
  `<img src="${ELSEWHERE}/beacon.png" onerror="window.${PWNED} = 'onerror'">`,
  '',
  `<a href="javascript:window.${PWNED} = 'raw anchor'">raw anchor</a>`,
  '',
  `[script link](javascript:window.${PWNED}='link')`,
  '',
  '[file link](file:///etc/passwd)',
  '',
  '[data link](data:text/html,hello)',
  '',
  `![tracking pixel](${ELSEWHERE}/pixel.png)`,
  '',
  '[the docs](https://example.com/docs?page=1&lang=en)',
  '',
  '[write to us](mailto:team@example.com)',
  '',
  '| left | right |',
  '| :--- | ----: |',
  '| a | b |',
].join('\n')
