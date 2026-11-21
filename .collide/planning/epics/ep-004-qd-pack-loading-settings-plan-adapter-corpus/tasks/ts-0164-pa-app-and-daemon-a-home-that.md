---
id: ts-0164-pa
kind: task
status: ready
title: "App and daemon: a HOME that is gone is made again"
owners: []
links: []
---

## Goal

In `ts-0138-pn` step 6 (2026-11-21) the owner started the app from a terminal that still held the check's `env.sh`,
whose `HOME` and `COLLIDE_DAEMON_ROOT` named a folder already deleted. The app started a daemon there, which made the
folder again, and ran with no word that its home was new. The app never saw the real daemon, so the page showed none of
the owner's runs, and it looked like a bug of the page.

## Success criteria

- The daemon refuses to start when `HOME` names no folder, and says so with the path; it never makes `HOME`.
- The app shows that refusal as its words, not as an empty page.
- `COLLIDE_DAEMON_ROOT` that names no folder is still made, as today (tests and checks make it fresh).

## Tests

- Write first and see red: the daemon binary with `HOME` set to a path that does not exist: it exits with the message,
  and the path is still absent.

## Notes

- Recommendation: as the criteria say. Alternative: start, and show the daemon's `HOME` on the page. Less safe: the
  wrong home still gets written.
- Low priority. The check's own text now says to look at `echo $HOME` before starting the app (`ts-0138-pn` step 6).
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.
