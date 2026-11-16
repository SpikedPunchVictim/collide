---
id: ts-0090-jd
kind: task
status: active
title: "ACP session: choosing the model"
owners: []
links:
  - { type: blocked-by, to: ts-0089-wv }
---

## Goal

The ACP session sets the model a role asked for, checks it against what the agent offers, and never reaches a model the
role did not name (0.3, Who picks what; S7 P6, O8, O9, O11). Capability `model_select` is `Session`.

**The id.** The session takes `ModelRef::adapter_id` (the opaque `model.id`, else `model.name`) from the request. The
id is only data: it is written in a JSON-RPC body and never in argv, env or a path, so it needs no safe token (the
`name` has one and is what the host puts in env and paths).

**After `session/new`.** The response may carry the model in two forms, and the session reads both:

- stable: `configOptions`, a list; the option with `category: "model"` holds `id`, `currentValue` and `options` (a flat
  list of `{ value, name }`, or groups of them);
- draft: `models`, with `currentModelId` and `availableModels` (`modelId`, `name`).

The session then:

1. finds the advertised ids, from `configOptions` if it has a model option, else from `models`, else none;
2. when the request has no model: writes nothing and keeps the agent's own default;
3. when the id is the current one: writes nothing;
4. when the id is advertised: writes `session/set_config_option` with the option's `configId` and the `value`, if the
   model came from `configOptions`; else `session/set_model` with the `modelId`;
5. waits for that response before it writes the first prompt (the prompt is held until the model is set), and checks the
   answer: for `set_config_option`, the response's `configOptions` hold the value as current; for `set_model`, an empty
   success is enough, and a later `current_model_update` or the next `session/new` must not contradict it;
6. when the id is **not** advertised, or the agent has no way to set a model: writes no prompt, writes nothing more,
   gives an `Other` named `model_unavailable` (the id is not in the event, only the count of models offered), and makes
   `finish` say the phase failed (see the decision below).

**An agent that sets the model itself.** S7 measured one that ignored the model in its config and used its own default
(O8). The session never trusts the config: it only trusts `currentValue` or `currentModelId` after its own write.

## Success criteria

- With `configOptions` that list the id, the bytes after `session/new` are `session/set_config_option` for the model
  option, with the id, and no `session/set_model`.
- With only the draft `models`, they are `session/set_model` for the id, and no `set_config_option`.
- With both forms in one response, `configOptions` wins.
- With a request that has no model, or whose id is already current, no model request is written, and the prompt follows
  at once.
- The first prompt is written only after the model request was answered. A model response that is an error writes no
  prompt and fails the phase.
- An id that is not in the list: no model request, no prompt, one `Other` named `model_unavailable`, a failed phase.
  This holds for an id that is another provider's hosted model (S7, O11) that the agent lists but the request did not
  ask for: the session never picks a model the request did not name.
- An agent that offers no model list and no way to set one, asked for a model, fails the phase the same way. Asked for
  none, it runs.
- A model id with `/` and `:` (`ollama/qwen2.5:7b`) is written as given in the JSON body. It is in no argv, env or file
  name that this adapter makes (`launch` is checked for this).
- Replayed from a journal, the session is in the same state, also when cut between the model request and its answer.
- The shared contract still passes.
- No new crate is added.

## Tests

Write these first:

- model in `configOptions`: flat list, grouped list, id current, id offered, id not offered, response with an error;
- model in the draft `models`: the same cases with `session/set_model`;
- both forms together;
- no model in the request; no list in the response; no list and a model asked for;
- the prompt held until the answer, and the order of the three writes after `session/new` (model, then prompt);
- the id with `/` and `:` in the body and nowhere in `launch`;
- replay cut between the model request and its answer;
- the fake child under the real shim, for each form: recordings that expect `session/set_config_option` or
  `session/set_model` with the id and answer it, then serve one turn; and one that offers other ids only (the phase
  fails and the child is stopped by the host).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3d: ACP engine (model through `configOptions` and `session/set_config_option`, `session/set_model` as the
    fallback).
  - 0.3: Who picks what (`model.id` beside `model.name`, checked against the advertised list); Capabilities
    (`model_select`); Later adapters; The ACP profile.
  - 0.5: safe token.
  - S7 record: P6 and G10, O8, O9, O11.
- **Decision for the owner, with a recommendation: how a refused model fails.** There is no `PhaseFailure` for it.
  Recommend a new variant, `ModelUnavailable`, with `failure_text` in `host/src/agent_feed.rs` saying that the agent
  does
  not offer the model the role names. A small change to `core/src/adapter.rs`, one arm in the host's failure text, and
  `pnpm gen`. Alternative: `Failed { cause: Api { status: None } }` with no new variant. Pro: no wire change. Con: the
  person sees "API failure" for a role that names a model the agent does not have, which is a pack or user-models
  mistake, not an outage.
- **Decision for the item, with a recommendation: where the id comes from.** Recommend that the check is the session's,
  not `launch`'s: the list exists only after `session/new`, so `launch` cannot do it. `launch` still refuses a model
  with no `ModelRef` for this tool (the role's alias is unmapped), as 0.3 says.
- **Decision for the item: the draft form.** The draft `models` shape is not in the stable schema. Keep it behind one
  small function with a comment that says when it can go (when the agents the profiles name all offer `configOptions`).
- The set-model request must not name a model the session did not read from the agent's own list, so a hosted model the
  agent lists (O11) is never chosen by a default.
- A `config_option_update` or a `current_model_update` arriving later stays an `Other`. The session does not follow an
  agent that changes its own model mid-session. A model change by the agent is the agent's, and the usage report names
  the model the turn used when the agent says one.
- Run `pnpm gen` if the variant is added.
- Area: `host` (`adapters/acp/session.rs`, `model.rs`), `core/src/adapter.rs` (the variant, if taken), the test folder.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3d item (revision 23 scope, split 2026-11-15). Items of Stage 3d run one at a time, in id order, because
  they share `host` and the generated files.
- Owner decisions (2026-11-15, at the split): the decisions for the owner above, taken as recommended under the
  owner's delegation; the owner may change them before the item is built. The ACP protocol is written by hand: no ACP
  crate is added.
