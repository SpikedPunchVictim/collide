// The provider's entry point: `node src/main.ts` speaks protocol v0 over stdio. The repository root is not read from
// the working directory: Collide sends it as `root` in `initialize`, and `list` and `get` fail until that has run.
import { servePlan } from './provider.ts'

await servePlan()
