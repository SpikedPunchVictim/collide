/**
 * The `yaml` package that Collide supplies with the SDK (0.6): a pack imports it as `@collide/pack-sdk/yaml`, as a
 * repo pack may hold no `node_modules` of its own. Only these two names are exported, so a pack's code has one way
 * to read YAML, and a new `yaml` is a new SDK version.
 */
export { parse, YAMLParseError } from 'yaml'
