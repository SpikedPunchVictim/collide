import { runAdd } from './add.js'
import { runList } from './list.js'
import { runRemove } from './remove.js'
import { runReport } from './report.js'
import { message } from './messages.js'

const commands = { add: runAdd, list: runList, remove: runRemove, report: runReport }

export async function main(argv) {
  const [name, ...rest] = argv
  const run = commands[name]
  if (!run) {
    console.error(message('unknown-command', { name }))
    return 2
  }
  try {
    await run(rest)
    return 0
  } catch (err) {
    console.error(message(err.code ?? 'unexpected', { detail: err.message }))
    return 1
  }
}
