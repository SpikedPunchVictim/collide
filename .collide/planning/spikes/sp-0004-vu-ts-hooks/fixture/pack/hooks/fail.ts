import { defineHook } from "@collide/pack-sdk";

defineHook(async (input, ctx) => {
  return ctx.fail(`gate failed for ${input.tasks.length} task(s)`);
});
