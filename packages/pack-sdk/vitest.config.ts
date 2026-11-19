import { defineConfig } from 'vitest/config'

// The generated-code check runs the generator over every schema, twice for the deterministic test;
// under the full gates (other suites running beside, a loaded machine) it overran vitest's 5 s
// default (6 s and 10 s seen, 2026-11-19). 30 s fits that work, is still bounded, and matches
// packs/plan's limit.
export default defineConfig({
  test: { testTimeout: 30_000, hookTimeout: 30_000 },
})
