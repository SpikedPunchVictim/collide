import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vitest/config'

// The Orchard pack's provider (fixtures/packs/orchard/providers) imports the SDK by its package name, as a repo pack
// must (0.6). A repo pack has no node_modules, so here the names point at the SDK's source, as Collide's cache links
// them to its compiled copy. The child-process test links the same files by hand.
const sdk = (file: string) => fileURLToPath(new URL(`../../packages/pack-sdk/src/${file}`, import.meta.url))

export default defineConfig({
  resolve: {
    alias: [
      { find: /^@collide\/pack-sdk\/yaml$/, replacement: sdk('yaml.ts') },
      { find: /^@collide\/pack-sdk$/, replacement: sdk('index.ts') },
    ],
  },
  // Some tests make a temp tree and a child process; under the full gates other suites run beside them.
  test: { testTimeout: 30_000, hookTimeout: 30_000 },
})
