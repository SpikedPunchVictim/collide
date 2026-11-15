// A tiny provider for the end-to-end test: it is run with `node`, which strips the types, and talks over real stdio.
import { defineProvider } from '../src/index.ts'

await defineProvider({
  initialize: ({ config }) => {
    console.error('provider script started')
    return {
      scope: 'global',
      idPattern: '^#\\d+$',
      shortIdPattern: '^\\d+$',
      statuses: [{ name: 'open', category: 'ready' }],
      capabilities: { watch: true, create: config.create === true },
    }
  },
  list: () => ({ tasks: [] }),
  get: ({ id }) => ({ task: { id, key: id, title: 'Task', version: '1', state: { category: 'ready', status: 'open' } } }),
  apply: () => ({ outcome: 'refused', message: 'read-only' }),
  watch: (_params, notify) => {
    notify('source.stale', { reason: 'no source' })
    return {}
  },
})
