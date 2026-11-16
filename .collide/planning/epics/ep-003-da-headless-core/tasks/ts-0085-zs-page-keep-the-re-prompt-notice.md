---
id: ts-0085-zs
kind: task
status: ready
title: "Page: keep the re-prompt notice across the run change"
owners: []
links: []
---

## Goal

After a re-prompt the page shows "Sent back. The agent is working on it again." (`SENT_BACK_WORDS` in
`app/src/loop/words.ts`). The daemon then makes a new run, the page follows it, and a run change clears the notice
(`afterRunChange` in `app/src/loop/state.ts`). When the second phase is quick, the person never sees the notice. Keep
it until the new run reaches its next state that needs the person, or ends.

## Success criteria

- After a re-prompt, the notice shows on the new run while it works.
- It goes when the new run reaches review, a permission request or an end, or when the person starts another prompt.
- Other run changes still clear what they cleared before.

## Tests

Write these first, in vitest over `FakeOwner`, as `app/test/loop-page.test.ts` does, and see them fail:

- a re-prompt followed by the new run's first events keeps the notice;
- the new run's review item clears it;
- a new prompt clears it.

## Notes

- Found by `ts-0082-fp` (its decision on the "Sent back" notice): the browser test asserts the second review
  instead of the notice.
- Area: `app/src/loop/state.ts`, `app/test/loop-page.test.ts`. No daemon or wire change.
- A dogfood candidate for the thin loop (Stage 3c, part L): small, and checked by eye on the page.
