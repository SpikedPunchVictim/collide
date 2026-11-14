import { defineHook, SDK_VERSION, SDK_URL } from "@collide/pack-sdk";
import type { Greeting } from "./lib/greet.ts";
import { greet } from "./lib/greet.ts";

interface Config {
  greeting: string;
  lanePool: number;
}

defineHook<Config>(async (input, ctx) => {
  const greeting: Greeting = await ctx.report.span("greet", async (span) => {
    span.metric("tasks.count", input.tasks.length, "count");
    return greet(ctx.config.greeting, input.tasks.map((task) => task.shortId));
  });
  ctx.report.progress(1, 1);
  return ctx.ok({
    text: greeting.text,
    lanePool: ctx.config.lanePool,
    cwd: process.cwd(),
    hookUrl: import.meta.url,
    sdkUrl: SDK_URL,
    sdkVersion: SDK_VERSION,
  });
});
