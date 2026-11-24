---
id: ts-0168-tq
kind: task
status: ready
title: "Provider contract: the board's source block, provenance and a stale cause"
owners: []
links:
  - { type: blocked-by, to: ts-0214-rz }
---

## Goal

A provider is told which GitHub repositories its board names and which repository its checkout came from. It can say who
wrote a task and how far that author is trusted. It can say why it is stale. The daemon and the corpus carry all three.
No GitHub code yet: the later items of Stage 5 build on this.

What exists. `core/src/provider.rs` has `InitializeParams { config, root }`, `SourceStale { reason, checkout }`,
`TaskChanged.actor` and `automated`, `ChangeCause::Remote` and `RemovedCause::Deleted` and `Transferred`. `Task` has
`fields`, `labels`, `kind` and no author. `core/src/board.rs` has `GithubSource` (`repos`), `BoardSources` and
`check_board`, which validate `sources.github.repos`, and nothing reads them for a provider. `host/src/sources.rs`
`plan` and `Source::start` give a provider only the pack's settings as `config`. `core/src/domain.rs` has
`Provenance { author, trust }` and `TrustLevel`, and `CachedTask` holds a provenance. `core/src/view_eval.rs` binds
`TaskVars::bind(task, kind, None)`, so a view never sees an author or a trust level from a source. `core/src/corpus.rs`
`mark_stale(source, reason)` keeps a source's last tasks and `reconcile_by_source` skips a stale source, so nothing is
removed on its account. `task.list` (`core/src/owner/task.rs` `StaleSource`) lists stale sources with the reason.

What this adds.
- `InitializeParams.board` (additive, optional): `{ repos: [<owner>/<repo>], origin: <owner>/<repo> | null }`. `repos`
  is `sources.github.repos` of the line's `board.yaml`, empty when there is none. `origin` is the GitHub repository
  that the checkout's `origin` remote names, read with git through `DaemonOptions::git_env` and parsed from https,
  `ssh://` and `git@host:` forms; `null` for no remote or a host that is not github.com. Every provider receives it and
  the `plan` pack ignores it.
- `Task.provenance` (additive, optional): `{ author?: string, association?: string }`, the source's own words (for
  GitHub, the login and `author_association`). The corpus's ingest caps both lengths. A new `trust_of(association)` in
  `core` gives `Trusted` for `OWNER`, `MEMBER` and `COLLABORATOR`, and `Untrusted` for every other value and for none.
  The cached task keeps the provenance, and `view_eval.rs` binds it instead of `None`, so a view expression reads the
  author and the trust level (0.9).
- `SourceStale.cause` (additive, optional): `auth`, `network`, `rate_limit`, `missing` or `other`. `Corpus::mark_stale`
  keeps it, and `StaleSource` in `task.list` carries it. A stale mark that a failed call makes (`failed_to_start`,
  `mark_source_stale` in `host/src/corpus_service.rs`) does not replace the reason and cause of a notification the
  source already gave. The source is fresh again when a read of it succeeds.
- The two `fields` names `launchable` (bool) and `launch_reason` (string) are the way a provider says a task cannot be
  launched. Core reads neither yet (Stage 9a launches). They are named here so that Stage 5 and Stage 9a agree.
- The schemas and the TypeScript types of `@collide/pack-sdk` are regenerated.

## Success criteria

- A provider's `initialize` params carry `board.repos` from the line's `board.yaml` and `board.origin` from the `origin`
  remote. A board with no `sources.github` block gets no repos. A checkout with no remote, or a remote on another host,
  gets a null origin.
- A task with `provenance.association` `OWNER`, `MEMBER` or `COLLABORATOR` is trusted in a view, and one with any other
  value, or with no provenance, is untrusted. A view expression can read the author.
- A `source.stale` with a cause shows in `task.list` with that cause, and a stale source keeps its tasks and loses none.
- A provider that sends none of the new fields behaves as before: every existing `plan` test passes unchanged.
- The schemas and the TypeScript types match the Rust types.

## Tests

Write these first.

- `core/tests/provider_protocol.rs`: a case for each new field, round-tripped, and an old payload without them (the
  `task.changed with actor` shape).
- A table test of `trust_of` for all eight `author_association` values: `OWNER`, `MEMBER`, `COLLABORATOR`,
  `CONTRIBUTOR`, `FIRST_TIME_CONTRIBUTOR`, `FIRST_TIMER`, `MANNEQUIN`, `NONE`, and a missing one.
- Corpus tests: provenance survives ingest and is capped; a stale cause is kept and cleared when the source is fresh;
  the first stale reason and cause stand over a later failed-call mark.
- A view-evaluation test in `core/tests/` over an expression on the author and on trust.
- Host tests with a fake provider (`host/tests/fixtures/`, the shape of `fake_person_only_provider.mjs`) that returns
  its `initialize` params in a task's body: a temp repository with `git remote add origin` (no network) for each URL
  form, and a table of the parse.
- `packages/pack-sdk/test/provider-types.test.ts` extended.

## Notes

- Area: `core/src/provider.rs`, `core/src/domain.rs`, `core/src/corpus.rs`, `core/src/view_eval.rs`,
  `core/src/owner/task.rs`, `host/src/sources.rs`, `host/src/corpus_service.rs`, the generated schemas and TypeScript,
  `host/tests/`.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test that
  runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp
  root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. No test, build or builder runs the real `gh`, reads a real token or reaches
  GitHub.
- Stage 5 items run one at a time, in id order, because they share `core`, `host`, `packs/` and the generated files.
  Nothing here runs a real `claude`. No dependency is added.
- Decisions with a recommendation, for the builder: the first stale mark stands until the source is fresh (the
  alternative, the last mark wins, hides a typed cause behind a failed call's plain text). The trust list is fixed
  (`OWNER`, `MEMBER`, `COLLABORATOR`), not a setting; it decides who may launch from an issue, so it is an Owner's
  choice in the proposal of the split.
- The board's repo is not implied: a board with a GitHub origin and no `sources.github` block shows no issues. An
  Owner's choice in the proposal; the recommendation is explicit, so that opening a board never starts network calls.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
