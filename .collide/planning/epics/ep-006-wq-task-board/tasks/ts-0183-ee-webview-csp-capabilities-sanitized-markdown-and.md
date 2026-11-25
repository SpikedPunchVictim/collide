---
id: ts-0183-ee
kind: task
status: done
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
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Built

- Capabilities. `app/src-tauri/build.rs` declares the app manifest, `AppManifest::new().commands(&["rpc"])`, which
  makes `tauri-build` write `permissions/autogenerated/rpc.toml` (`allow-rpc`, `deny-rpc`). `capabilities/main.json`
  (`main-window`) grants `allow-rpc`, `core:event:allow-listen` and `core:event:allow-unlisten`, and nothing else. The
  generated folder is in `.gitignore` (see Decisions).
- CSP. `tauri.conf.json` `app.security.csp` is a map of directives, so each line stays under 120 columns: `default-src
  'none'`, `script-src 'self'`, `style-src 'self'`, `img-src 'self' data:`, `font-src 'self'`, `connect-src ipc:
  http://ipc.localhost`, `object-src 'none'`, `base-uri 'none'`, `form-action 'none'` and `frame-ancestors 'none'`. No
  remote origin, no `'unsafe-inline'`, no `'unsafe-eval'`. `app.security.devCsp` is the same plus `'unsafe-inline'`
  for scripts and styles and loopback http and ws in `connect-src` (see Decisions). `app/src-tauri/src/csp.rs` (new)
  reads the file and has `directives`, `built` (the header value) and `for_bridge_page`; its module comment lists each
  directive and why.
- Bridge. `http.rs` `write_response` takes extra headers. `main.rs` sends the page with `Content-Security-Policy`
  (`csp::for_bridge_page`: the built policy with `'self'` first in `connect-src`, because the page calls the bridge
  over http at its own origin where the app uses Tauri's channel; nothing else differs). `Bridge` has a
  `RecordingOpener`, and `GET /opened` (token checked) lists the links `shell.open_external` was asked to open.
- Shell. `shell.rs`: `SHELL_PREFIX`, `OPEN_EXTERNAL`, the `Opener` trait, `UsrBinOpen` (runs `/usr/bin/open` with the
  link as one argument, null stdio, a reaper thread named `collide-open` so the shell thread never waits),
  `open_command` (builds the command and never runs it in a test), `RecordingOpener`, `check_link` (only `http://`,
  `https://` and `mailto:` in any case, at most 2048 bytes, no control or white-space character; an error never
  repeats the link) and `open_external`. `Shell::new` takes the opener as its third argument, and `lib.rs`
  `build_shell` is the one place that makes `UsrBinOpen`. `Shell::rpc` answers `shell.open_external` locally and
  returns method-not-found for any other `shell.*` method, so none is forwarded.
- App. `app/src/markdown/` (new): `links.ts` (`openableLink` reads the address as a browser does and keeps only http,
  https and mailto; `schemeOf`), `Markdown.tsx` (`react-markdown` and `remark-gfm`, no `rehype-raw`; a custom
  `urlTransform` and components for `a`, `img` and `h1` to `h6`; a heading is always an `h4`; a kept link is a
  `<button>` with no `href`; a refused link is its text and "(link not opened: javascript: links are never opened)";
  an image is "[image not loaded: alt, from url]"), and `ExternalLinks.tsx` (`ExternalLinks` provider, `useAskToOpen`,
  `LinkWindow`: the browser's `<dialog>` with `showModal`, Cancel first, "Open in browser" calls
  `shell.open_external`, a failure shows as an alert and keeps the window). `App.tsx` wraps the window in
  `ExternalLinks`. `RunCard.tsx` (the prompt) and `Queue.tsx` (a request's reason) use `Markdown`: they are the only
  places that show task or agent text before the drawer. `index.css` has the `.md` rules. `owner/methods.ts` has
  `SHELL_METHODS`, typed by hand, and `client.ts` and `index.ts` carry it.
- Fake-owner page. `browser/fake/main.tsx` answers `shell.open_external` by recording it, and
  `window.collideFake.opened()` reads the record. `browser/support/fake-site.ts` sends the built CSP
  (`browser/support/csp.ts` reads it from `tauri.conf.json`). `test/support/worlds.ts` has the `hostile-text` world (a
  task whose body is the hostile text, with a run and a permission request whose reason is the same text), and
  `test/support/hostile.ts` has the body. Its addresses are loopback on the discard port, so nothing leaves the
  machine even if a payload loaded.
- Tests, Rust, `shell.rs` unit: `an_accepted_link_is_passed_to_the_opener_as_one_argument`,
  `a_link_with_another_scheme_is_refused_and_nothing_is_opened`,
  `a_link_with_a_newline_a_space_or_a_control_character_is_refused`,
  `a_link_over_the_limit_is_refused_and_one_at_it_is_not`, `a_refusal_does_not_repeat_the_link`,
  `params_that_are_not_one_url_are_refused`, `an_opener_that_fails_gives_an_error_that_says_why`,
  `the_real_opener_runs_usr_bin_open_with_the_link_as_its_one_argument` (it checks the command that is built and does
  not run it). `tests/rpc.rs`: `shell_open_external_is_answered_locally_and_the_forward_path_is_not_called`,
  `shell_open_external_refuses_a_scheme_it_does_not_open_and_does_not_forward_it`,
  `an_unknown_shell_method_is_refused_here_and_is_not_forwarded`. `collide-bridge` `http.rs`:
  `a_response_carries_the_extra_headers_it_is_given`. `tests/security.rs` (new, 11 tests) reads
  `capabilities/main.json`, `tauri.conf.json` and `build.rs`: the permissions are `allow-rpc`, listen and unlisten;
  one capability file; the manifest names `rpc`; the built policy is the strictest and has no remote origin or unsafe
  source; a policy with a remote origin, `'unsafe-inline'` or no policy at all is found; the dev policy adds only
  loopback and inline and the built one has neither; the header is one line; the bridge page differs in one source.
  The old capability test in `tests/rpc.rs` is gone: it is replaced by the first of these.
- Tests, app: `app/test/markdown.test.tsx` (43 tests: headings, lists, code, tables with alignment, emphasis; raw HTML
  as text; each refused scheme; relative and anchor links; images; the hostile body, checked by the tags and
  attributes of the markup), `app/test/link-window.test.tsx` (7), and `app/test/owner-methods.test.ts` (the shell
  method names match `shell.rs`). Browser: `app/browser/fake-hostile.spec.ts` (4: the hostile body, with no request
  outside the page's origin, no dialog, the flag unset and no policy error in normal use; an `<img>` and an inline
  `<script>` added by the test raise `securitypolicyviolation`, and the request fails for `csp`; Cancel and Escape
  open nothing and Open records the one address; a mail link names no host), `app/browser/fake-site.spec.ts` (the page
  carries the built policy), and `app/browser/bridge.spec.ts` (the page carries the policy; `shell.open_external`
  refuses `javascript:`, `file:`, `data:`, a space and a relative link with -32602, accepts https and mailto, and the
  bridge records only those; `/opened` needs the token).
- Seen red at runtime. Rust: against a `check_link` that accepts everything, four shell unit tests failed (another
  scheme, newline and control characters, the length limit, the refusal repeating the link); with `open_command`
  naming the wrong program, the real-opener test failed; with `shell.*` routing removed, the three new `rpc.rs` tests
  failed; with the old configuration, capability and `build.rs` restored, eight of the ten first `security.rs` tests
  failed, and `the_bridge_page_differs_...` failed against a `for_bridge_page` that returns the built policy. The
  accepted-link unit test passes before and after: it guards the green path. App: against stubs of `Markdown` and
  `links.ts`, 40 of the 43 markdown tests failed; a `LinkWindow` with no host line, no address line and no `disabled`
  failed three of the link window tests. Browser: with the fake site sending no policy, the `<img>` and `<script>`
  test and the fake-site policy test failed.

### Decisions for the owner

- Where the dev app gets its policy. Tauri 2 sends `csp` as a header on responses of its own `tauri://` protocol only,
  and adds nonces and hashes to `script-src` and `style-src` itself. With `devUrl` pointing at the Vite dev server,
  the page comes from Vite, which sends no CSP, so no policy applies there, and `devCsp` (used only in dev, falling
  back to `csp`) cannot tighten it. I set `devCsp` anyway, with loopback http and ws and `'unsafe-inline'`, so a dev
  run that does use it works, and kept all of that out of `csp`; a test keeps them apart. Recommendation: keep.
  Alternative: drop `devCsp` and let dev fall back to `csp`, which would block hot reload if it applied. On first
  launch of the built app, check: the console shows no policy violation; the window loads and styles; a call and an
  event work (the board loads and updates). I could not launch it. If `rpc` or events fail, `connect-src` is the
  place: `ipc: http://ipc.localhost` is Tauri's documented minimum.
- `default-src 'none'` with each directive named, over `default-src 'self'`. It blocks frames, workers and media by
  default. Alternative: `'self'`, which is shorter and allows a same-origin frame. Recommendation: keep `'none'`.
- The bridge page's policy adds `'self'` to `connect-src`. The bridge page cannot call the bridge otherwise. The fake
  page has no network and gets the built policy exactly. Alternative: a test-only policy with more. Recommendation:
  keep, since the difference is one source and a test checks it.
- A kept link is a `<button>`, not an `<a>`. It has no `href`, so the webview has nothing to navigate to even if a
  click handler were missed; the browser test finds no `a[href]` on the page. Cost: no right-click "copy link" and no
  hover address (the address is in `title`). Alternative: an `<a>` with `preventDefault`. Recommendation: keep.
- A refused link and an image show as the mock's text, in muted italics. A link with no scheme (a relative path) is
  refused with "it has no web address". A `#anchor` link shows only its text. A heading in a body is an `h4`, so a
  body cannot change the outline of the page. Titles stay plain text, as the item says nothing of them.
- The link window is the browser's `<dialog>` with `showModal()`, not Radix Dialog. It holds focus and closes on
  Escape with no library, and it adds no inline style, which `style-src 'self'` would not allow if a library injected
  one. Cancel is first and has the focus, so Enter does not open. The window's words are the mock's, with two changes:
  it does not say "this task's text" because the text may be an issue or an agent's, and the button says "Open in
  browser" where the item says "Open", so the button names what it does. Alternative: Radix Dialog, for the look of
  the board menu. Recommendation: keep the native one.
- `shell.open_external` also refuses a link over 2048 bytes and any control or white-space character, not only a wrong
  scheme. A real browser address has none, and a space or a newline is how a second argument or a second line would be
  smuggled. `/usr/bin/open` runs with null stdio and its exit is reaped by a thread, so a slow `open` cannot stall the
  shell. An unknown `shell.*` method answers method-not-found and is never forwarded, so the shell's names cannot be
  used to reach the owner. `shell.pick_folder` is not implemented: it is not in this item.
- The types of `SHELL_METHODS` are written by hand, and a test checks the name against `shell.rs`. The method is the
  shell's, not the owner's, so it has no schema in `core`. Alternative: a type in `core` and `pnpm gen`, which would
  put a shell method in the owner's wire. Recommendation: keep by hand until a second shell method exists.
- `app/src-tauri/permissions/autogenerated/` is in `.gitignore`. `tauri-build` writes it at every build from the app
  manifest. Alternative: commit it, as plugins do. Recommendation: ignore, as `gen/` is.
- Plan text to apply, for the plan's owner: in the Tauri shell bullet of Stage 6a, "the shell answers `ui_tools.*` and
  `shell.*` methods itself and never forwards them", and "the bridge's page carries the app's policy with `'self'`
  added to `connect-src`".

### Not done, by design

- The check in the bundled app's webview (the policy in force, no other command callable, a link opening in the real
  browser) is the owner's, in `ts-0185-ge` and `ts-0197-uj`. No builder runs `tauri build`, and no test opens a link.
- The drawer (`ts-0184-hv`) is the first place to show a task's body. It uses `Markdown` and the provider; this item
  has no other place to wire.
- The GitHub-issue form of the hostile fixture is Stage 5's `ts-0176-ds`.
- A right-click menu with copy link, and a hover card with the address, for a kept link.
