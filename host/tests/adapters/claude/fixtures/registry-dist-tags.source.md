# registry-dist-tags.json

The shape of the npm registry's answer for `@anthropic-ai/claude-code`'s dist-tags (ts-0068-wx): one JSON object of tag
to version. The values are the ones the plan records as measured on 2026-11-15 (`latest` 2.1.325, `stable` 2.1.311),
plus a `next` tag the chip ignores. The file is written by hand to that shape: the build fetched no registry, and no test
does. The tests serve it from a loopback address.
