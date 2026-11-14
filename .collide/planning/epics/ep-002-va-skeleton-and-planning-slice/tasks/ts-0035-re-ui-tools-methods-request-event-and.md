---
id: ts-0035-re
kind: task
status: ready
title: "ui_tools methods, request event and manifest"
owners: []
links:
  - { type: blocked-by, to: ts-0032-pp }
---

## Goal

The shell's `ui_tools.*` contract and its protocol answers. Types: the methods `ui_tools.reply`, `ui_tools.token` and `ui_tools.rotate`; the event `ui_tools.request {requestId, caller, deadline, message}` (`caller` is the ask id or `external`; `deadline` is when the shell gives up, 10 s); and the build-time tool manifest's file format (a JSON file of `{name, description, inputSchema}` entries, as MCP's `tools/list` returns, with its own JSON Schema). The five tools' definitions are TypeScript in the window and are written in Stage 6 and the later stages, together with the generator that makes the manifest from them. A pure request handler in `host` (a `ui_tools` module, no HTTP listener until Stage 6b): it refuses a foreign `Host` or `Origin`, a content type that is not JSON, a body over 1 MB, and a missing or wrong token (constant-time compare); answers `initialize`, `ping` and `tools/list` from the manifest; returns 202 with no body for a notification and 405 for `GET` and `DELETE`; forwards only `tools/call`, as `ui_tools.request`; ignores a reply to a request it dropped; counts the per-ask cap of 20 screen-changing calls itself; and tells the caller `Collide has no window open; open Collide and ask again`, or `the Collide window did not answer within 10 s`, in the two failure cases. It emits `ask.cancel` for every live ask when the window closes or reloads, and it handles `ui_tools.*` methods itself and never forwards them.

## Success criteria

- With a fake window not connected, `initialize`, `ping` and `tools/list` answer from the manifest.
- A notification gets 202 and no body. `GET` and `DELETE` get 405.
- A `tools/call` becomes one `ui_tools.request` with `requestId`, `caller`, `deadline` and `message`, and a `ui_tools.reply` with that `requestId` completes it. A late reply is ignored.
- The 21st screen-changing call in one ask is refused, and a simulated window reload does not reset the count.
- A foreign `Host`, a non-JSON content type, a body over 1 MB and a wrong token are each refused.
- With no window, `tools/call` fails with the exact text above. A silent window gives the 10 s text.
- Closing the window yields `ask.cancel` for each live ask, and a new window starts with none.
- `ui_tools.token` and `ui_tools.rotate` never reach the forward queue.
- A manifest fixture validates against the manifest schema, and an invalid one fails.

## Tests

Write first: a handler test per bullet above, with a fake clock and a fake window; the manifest schema test.

## Notes

- IMPLEMENTATION_PLAN.md, 0.2 (The UI-tools endpoint, its guards, tokens and the window-closed rule), 0.9 (an agent's tool inputs are untrusted; the cap of 20), 0.11 (the five tools), Stage 2: Skeleton, Contracts and Success Criteria (the `ui_tools.request` round trip).
- Area: `host` (`ui_tools` module), `core` (types and schemas).
- Placement note: the plan keeps `app/src-tauri` thin, so the handler lives in `host` and the shell calls it. The HTTP listener and the endpoint token's keychain storage are Stage 6b.
- Done when the criteria hold.
