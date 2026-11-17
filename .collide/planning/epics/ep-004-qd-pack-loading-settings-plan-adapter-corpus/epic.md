---
id: ep-004-qd
alias: stage-4
kind: epic
status: active
title: "Stage 4: Pack loading, settings, plan adapter, corpus and views"
owners: []
links: []
---

## Goal

Tasks come from a ref-scoped source that can be read and written, are combined into a corpus, and are filtered by views. Everything loads from a trusted snapshot, with the pack's settings.

## Success criteria

- Stage 4a: Packs, trust and settings. IMPLEMENTATION_PLAN.md, Stage 4a, Success Criteria.
- Stage 4b: Providers, plan adapter, corpus and views. IMPLEMENTATION_PLAN.md, Stage 4b, Success Criteria.
- Stage 4c: Planning. IMPLEMENTATION_PLAN.md, Stage 4c, Success Criteria. It adds the `spike` kind, so the planning slice reads `spikes/` from here.

## Notes

Stage 4a is split (2026-11-16): `ts-0093-am` to `ts-0105-jv`, built one at a time in id order. The contracts and a YAML writer come first, then the git reader, the materializer and the loader, then trust, the settings layers, agent entries and the keychain, `git:` sources and `pack update`, user packs, the daemon and CLI, and last the repository instructions: measured, built, and checked by the owner. Stage 4c is split when it starts (0.10, 0.15).

Stage 4b is split (2026-11-17): `ts-0108-dq` to `ts-0120-wm` are built one at a time in id order, and `ts-0121-xp` is the owner's check. The provider supervisor and the SDK's hooks come first, then the plan adapter (read, write, many checkouts), the daemon's sources, the write guards, the corpus (core, then daemon), CEL, views (core, then daemon), the CLI, and last the owner's check on the Orchard repo.
