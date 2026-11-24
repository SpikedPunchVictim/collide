---
id: ts-0183-ee
kind: task
status: ready
title: "Webview: CSP, capabilities, sanitized markdown and external links"
owners: []
links:
  - { type: blocked-by, to: ts-0182-ad }
---

## Goal

The webview gets its content security policy and its last capabilities, and task, issue and agent text is rendered
as markdown that cannot run, fetch or navigate. An external link opens in the person's browser only after they
confirm it. The drawer (`ts-0184-hv`) is the first place that shows a task's body, so this lands before it.

What exists.
- `app/src-tauri/tauri.conf.json` sets no `app.security.csp`, so the webview runs with none.
- `app/src-tauri/capabilities/main.json` (`main-window`) grants `core:event:allow-listen` and `allow-unlisten` only.
  `app/src-tauri/build.rs` calls `tauri_build::build()` with no app manifest, so the app's own `rpc` command is not
  named in any capability.
- `app/src-tauri/src/shell.rs`: the one `rpc` command and the one event channel. A `ui_tools.*` method is answered
  in the shell by `UiToolsEndpoint::rpc` and never forwarded; every other method goes to the owner with the token.
- The bridge (`app/src-tauri/src/bin/collide-bridge/http.rs`) serves the page with an origin and token check
  (`Policy`) and sends no CSP.
- Text is shown as plain text everywhere (`app/src/loop/`). No markdown library is in `app/package.json`.

What this adds.
- A CSP in `tauri.conf.json` (`app.security.csp`): the strictest the built app runs under, with no remote origin.
  Images from `self` and `data:` only (the app's own icons), no `connect-src` beyond what Tauri's IPC needs, no
  inline script, `object-src 'none'`, `base-uri 'none'`, `form-action 'none'`, `frame-ancestors 'none'`. The builder
  lists each directive and why in the module comment.
- Capabilities: an app manifest in `build.rs` (`tauri_build::Attributes` with `AppManifest::commands(&["rpc"])`),
  and `main-window` granting the `rpc` command and the event channel's listen and unlisten, and nothing else.
- The bridge and the fake-owner page (`ts-0179-ke`) send the same CSP as a header, so the browser tests run under
  it.
- A `Markdown` component that renders to React elements, never to HTML strings. Raw HTML in the text shows as text.
  Links keep only `http:`, `https:` and `mailto:`; any other scheme shows as text. Images are never loaded: an image
  shows as its alt text and URL. Every place in the app that shows task, issue or agent text uses it.
- External links (R5): a click opens a small window naming the full URL, with Open and Cancel (the mock's
  `extModal`). Open calls a shell-local method, `shell.open_external`, answered in `shell.rs` beside `ui_tools.*`
  and never forwarded. It accepts only `http:`, `https:` and `mailto:`, and runs `/usr/bin/open` with the URL as one
  argument. In the bridge and the fake-owner page, the method is answered locally and records the call.

## Success criteria

- A fixture task whose body holds raw HTML, a `javascript:` link, a `file:` link and an image with a remote URL is
  rendered as text: nothing runs, nothing is fetched, and the CSP would block the image (Stage 6a; the GitHub-issue
  form is Stage 5's `ts-0176-ds`).
- An external link asks before it opens, names the full URL, and a Cancel opens nothing (R5).
- `shell.open_external` refuses any scheme but `http`, `https` and `mailto`, and never reaches the owner.
- The capability file grants `rpc`, listen and unlisten, and nothing else; the CSP has no remote origin.

## Tests

Write these first.

- `app/test/markdown.test.tsx`: headings, lists, code, tables; raw HTML as text; each refused scheme; an image as
  text; a link keeps its text.
- `app/browser/fake-hostile.spec.ts` (fake-owner page, under the CSP): the hostile body; the page makes no request
  outside the page's origin (Playwright's request log), no dialog opens, no script runs (a `window` flag the payload
  would set stays unset); an `<img>` added to the page by the test raises a `securitypolicyviolation` event. The
  link window opens on click and Cancel closes it.
- `app/src-tauri/src/shell.rs` unit tests: `shell.open_external` is answered locally, refuses `javascript:`,
  `file:`, `data:` and a URL with a newline, and passes an accepted URL as one argument to a fake opener.
- `app/src-tauri/tests/`: reads `capabilities/main.json` and `tauri.conf.json` and checks the permissions and that
  the CSP names no remote origin.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: the Tauri shell bullet (CSP and capabilities; 0.9; R5); Markdown in the webview. Success Criteria: the
    hostile fixture task. Tests: "the hostile-body fixture in a browser, against the CSP".
  - 0.9: the webview's rules. R5. 0.2: the shell forwards to the owner and adds the token.
- Design: the mock's `extModal` (the confirm window) and `mdHtml` (what markdown is shown). Mock functions:
  `extModal`, `mdHtml`.
- **Owner's choice, recommendation taken (2026-11-24): the markdown renderer.** Recommend `react-markdown` 10.1.0 with
  `remark-gfm` 4.0.1, new npm dependencies with no `rehype-raw`: tested, renders to React elements, raw HTML off by
  default, and tables and task lists as the plan's files use them. Alternative: a hand-written subset like the mock's
  `mdHtml` (no dependency; headings, lists, code, links, tables), which is more code to keep safe.
- **Decision for the item, with a recommendation: how a link opens.** Recommend `/usr/bin/open` from the shell, with
  the scheme checked first: no new dependency, and the capability list stays at `rpc`. Alternative:
  `tauri-plugin-opener` (a new crate and a capability; an Owner's choice if taken).
- **Decision for the item, with a recommendation: naming the shell's own methods.** Recommend the `shell.` prefix
  for methods answered in the shell, like `ui_tools.`, and the plan's text noting them (see the split's text to
  apply). Alternative: a second Tauri command; it breaks "one `rpc` command".
- The check in the bundled app's webview (the CSP in force, no other command callable) is the owner's, in
  `ts-0185-ge` and `ts-0197-uj`. No builder runs `tauri build`.
- Area: `app/src-tauri/tauri.conf.json`, `app/src-tauri/capabilities/main.json`, `app/src-tauri/build.rs`,
  `app/src-tauri/src/shell.rs`, `app/src-tauri/src/bin/collide-bridge/`, `app/src-tauri/tests/`,
  `app/src/` (a `Markdown` component and the link window), `app/browser/`, `app/test/`, `app/package.json`.
- Dependencies: `react-markdown` 10.1.0 and `remark-gfm` 4.0.1 (owner, 2026-11-24).
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile
  (`app/browser/support/chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`). No
  test opens a real browser window through `/usr/bin/open`: the opener is a seam.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
