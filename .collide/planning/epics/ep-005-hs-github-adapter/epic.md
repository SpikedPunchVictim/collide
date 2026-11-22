---
id: ep-005-hs
alias: stage-5
kind: epic
status: ready
title: "Stage 5: GitHub adapter"
owners: []
links: []
---

## Goal

A global source with live updates and writes over Issues (REST), on the same provider protocol, before the UI depends on it. Projects v2 is not here: it is Stage 6c, in epic `ep-006-wq`.

## Success criteria

- IMPLEMENTATION_PLAN.md, Stage 5: GitHub adapter, Success Criteria (Issues only).

## Notes

Stage 5 has no lettered sub-stages. It is split (2026-11-21): `ts-0168-tq` to `ts-0176-ds` are built one at a time
in id order, and `ts-0177-hd` is the owner's check on real GitHub. The provider contract, the per-user setting and
the daemon's global-source paths come first, on a fake provider; then the `github` pack grows read by read (client
and token, list, watch, writes); then the Stage 5 criteria run through the daemon on fakes.
