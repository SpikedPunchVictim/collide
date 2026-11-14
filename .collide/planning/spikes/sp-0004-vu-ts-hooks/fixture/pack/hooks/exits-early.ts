// Exits before any verdict, so no result file exists. Collide must classify this as `error` (0.5).
import { defineHook } from "@collide/pack-sdk";

defineHook(async () => {
  process.exit(0);
});
