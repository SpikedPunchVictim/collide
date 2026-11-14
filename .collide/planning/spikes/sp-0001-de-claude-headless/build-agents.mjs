#!/usr/bin/env node
// Builds the JSON object that `claude --agents <file>` takes from .claude/agents/*.md-style
// definitions (YAML-ish frontmatter + body as the prompt). Handles only the flat
// `key: value` frontmatter the S1 fixture uses; a real adapter would use a YAML parser.
//
// Usage: node build-agents.mjs <out.json> <definition.md>...
import { readFileSync, writeFileSync } from 'node:fs';

const [out, ...defs] = process.argv.slice(2);
if (!out || defs.length === 0) {
  process.stderr.write('usage: build-agents.mjs <out.json> <definition.md>...\n');
  process.exit(2);
}

const agents = {};
for (const file of defs) {
  const text = readFileSync(file, 'utf8');
  const m = text.match(/^---\n([\s\S]*?)\n---\n([\s\S]*)$/);
  if (!m) throw new Error(`${file}: no frontmatter block`);
  const front = Object.fromEntries(
    m[1].split('\n').filter(Boolean).map((line) => {
      const i = line.indexOf(':');
      return [line.slice(0, i).trim(), line.slice(i + 1).trim()];
    }),
  );
  if (!front.name || !front.description) throw new Error(`${file}: name and description are required`);
  const agent = { description: front.description, prompt: m[2].trim() };
  if (front.model) agent.model = front.model;
  if (front.tools) agent.tools = front.tools.split(',').map((t) => t.trim());
  agents[front.name] = agent;
}
writeFileSync(out, JSON.stringify(agents, null, 2) + '\n');
