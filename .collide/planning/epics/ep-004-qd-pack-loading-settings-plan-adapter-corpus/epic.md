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

Stage 4a is split (2026-11-16): `ts-0093-am` to `ts-0105-jv`, built one at a time in id order. The contracts and a YAML writer come first, then the git reader, the materializer and the loader, then trust, the settings layers, agent entries and the keychain, `git:` sources and `pack update`, user packs, the daemon and CLI, and last the repository instructions: measured, built, and checked by the owner. Stages 4b and 4c are split when they start (0.10, 0.15).
