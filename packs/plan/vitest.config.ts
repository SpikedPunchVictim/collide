import { defineConfig } from 'vitest/config'

// Many tests here make real git repositories and run the provider in a loop; under the full
// gates (other suites running beside) they pass vitest's 5 s default only barely. 30 s fits that
// work, is still bounded, and matches the per-test limit the process tests already use.
export default defineConfig({
  test: { testTimeout: 30_000, hookTimeout: 30_000 },
})
