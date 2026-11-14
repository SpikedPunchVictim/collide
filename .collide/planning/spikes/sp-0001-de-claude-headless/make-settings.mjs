#!/usr/bin/env node
// Prints a Claude settings JSON whose hooks append "<Event> tag=<TAG> ..." lines to $S1_MARKS.
// Every hook is harmless: it only appends to that file (or /dev/null when S1_MARKS is unset).
// The SessionStart line also lists the names (not values) of the CLAUDE* / ANTHROPIC_*
// variables the hook inherited.
//
// Usage: node make-settings.mjs <TAG> [extra-settings-json]
const [tag, extra = '{}'] = process.argv.slice(2);
if (!tag) {
  process.stderr.write('usage: make-settings.mjs <TAG> [extra-settings-json]\n');
  process.exit(2);
}
const sink = '"${S1_MARKS:-/dev/null}"';
const line = (event) => `echo "${event} tag=${tag}" >> ${sink}`;
const hook = (command, matcher) => [{ ...(matcher ? { matcher } : {}), hooks: [{ type: 'command', command }] }];

const settings = {
  hooks: {
    SessionStart: hook(
      `echo "SessionStart tag=${tag} env=[$(env | cut -d= -f1 | grep -E '^(CLAUDE|ANTHROPIC_)' | sort | tr '\\n' ' ')]" >> ${sink}`,
    ),
    UserPromptSubmit: hook(line('UserPromptSubmit')),
    PreToolUse: hook(line('PreToolUse'), '*'),
    Stop: hook(line('Stop')),
    // One echo per hook, so lines from hooks that run at the same time do not interleave.
    InstructionsLoaded: hook(
      `echo "InstructionsLoaded tag=${tag} $(jq -c '[.memory_type, .load_reason, .file_path]')" >> ${sink}`,
    ),
  },
  ...JSON.parse(extra),
};
process.stdout.write(JSON.stringify(settings, null, 2) + '\n');
