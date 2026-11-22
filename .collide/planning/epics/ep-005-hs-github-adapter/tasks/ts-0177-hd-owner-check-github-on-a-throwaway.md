---
id: ts-0177-hd
kind: task
status: ready
title: "Owner check: GitHub on a throwaway repository"
owners: []
links:
  - { type: blocked-by, to: ts-0176-ds }
---

## Goal

The Stage 5 checks that only the owner runs, against the real GitHub, on a throwaway repository: the 60 s edit, a real
close, a real delete and transfer, a real expired token and a real scope failure. It is also the one place the
hand-written fixtures of `ts-0172-xc` are compared with GitHub's real responses.

A build agent never runs it. It never touches the owner's real repositories, never reaches the network, never runs the
real `gh`, never reads a token, never starts a daemon against the owner's data, and never runs `claude`. It writes the
command lines and a short checklist under `### Owner checklist`, from the code and from the Built sections of
`ts-0168-tq` to `ts-0176-ds`. The owner runs them and records the result here as owner judgement, with what was
awkward.

The repository is a throwaway of the owner's own, ideally private, made for the check and deleted after, with a second
throwaway for the transfer. It is never this public repo's issues. The `gh` account is the owner's, or a second account
made for it. A scratch board (a temp clone) names the throwaway in `sources.github.repos`.

The owner's run is also where a difference between a fixture and a real response is recorded. Each becomes a fix item.

## Success criteria

- An issue edited on GitHub (its title, a label) appears on the board within 60 s at the default `pollSeconds`, measured
  with a stopwatch, three times. Owner-run.
- `setStatus` with category `done` through `collide task` closes the real issue, and the same with a version that
  another edit made stale returns a conflict before the write. Owner-run.
- An issue deleted by the repository's admin leaves the corpus within one reconcile interval (set `reconcileSeconds`
  small for the check, then once at the default). An issue transferred to the second throwaway also leaves
  it. Owner-run.
- A classic token without `repo` on a private throwaway names the missing scope and the fix at start. Owner-run.
- An expired or revoked token at start makes the source "could not check": the cached issues stay, one Queue item says
  to sign in again, and Retry after `gh auth login` clears it. Owner-run.
- A pull request on the throwaway is listed and read-only; an issue of a repo that is not the checkout's origin is
  listed and says it cannot be launched. A view over the real plan items and the issues lists both. Owner-run.
- The fixtures of `ts-0172-xc` match the real responses, or each difference is written down. Owner-run.
- After ten minutes of watching, the rate-limit remaining is recorded, to show that a 304 poll costs nothing. Owner-run.

## Tests

None. The owner's run is the test.

## Notes

- The test rule for every item of Stage 5 applies to the builder who writes the checklist: no test touches the real
  home, real launchd, a real daemon, the keychain or the network. Tests that run the `collide` binary clear the
  environment (`env_clear()`) and set `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root.
- Stage 5 items run one at a time, in id order. This is the last. Nothing here runs a real `claude`. No dependency is
  added.
- The checklist is written after `ts-0176-ds` is done, so it names the real commands and settings.
- Owner decisions: none recorded yet. The repository choice is an Owner's choice in the proposal of the split; the
  recommendation is a throwaway repository of the owner's own.
