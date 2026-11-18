---
title: "Harbor plan"
status: active
goal: "Made-up goal: the harbor has a price list."
---

Made-up epic. Its tables name the tasks.

<!-- BEGIN GENERATED tasks -->
| ID | Status |
|---|---|
| [T-03-001](../../proposals/03-harbor-index/tasks/T-03-001-survey-the-quay.md) | done |
| [T-03-002](../../proposals/03-harbor-index/tasks/T-03-002-map-the-moorings.md) | doing |
| [T-03-003](../../proposals/03-harbor-index/tasks/T-03-003-price-the-berths.md) | ready |
| [T-03-005](../../proposals/03-harbor-index/tasks/T-03-005-retire-the-old-ledger.md) | dropped |
<!-- END GENERATED -->

## The run, in slices

Prose that names T-05-001 is not a table row and joins no group.

### Slice 1: first berths

| Lane | Tasks, in order | Waits on another lane |
|---|---|---|
| North | T-03-001 → T-03-002 | none |

### Slice 2: the price list

| Lane | Tasks, in order | Waits on another lane |
|---|---|---|
| South | T-03-003, then T-05-003 | Slice 1 |

### Gaps

This section is not a slice, so its table joins no group.

| Gap | Tasks |
|---|---|
| 1 | T-03-004 |

```
### Slice 9: inside a code fence

| Lane | Tasks, in order | Waits on another lane |
|---|---|---|
| East | T-05-001 | none |
```

### The harbor streams

| Stream | Owns | Gate | PoC |
|---|---|---|---|
| **S1: quay work** | T-03-001, T-03-002 (M) | T-03-001 | T-03-002 |
| **S2: pricing** | T-03-003 (S), T-05-003 | T-03-003 | none |
