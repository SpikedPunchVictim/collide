// The Orchard pack's provider: `node providers/plan.ts` speaks protocol v0 over stdio. Collide sends the checkout's
// root in `initialize`; nothing is read from the working directory.
import { servePlan } from './plan/provider.ts'

await servePlan()
