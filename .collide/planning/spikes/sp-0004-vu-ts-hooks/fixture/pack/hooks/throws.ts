import { defineHook } from "@collide/pack-sdk";

defineHook(async () => {
  throw new Error("boom: the handler threw");
});
