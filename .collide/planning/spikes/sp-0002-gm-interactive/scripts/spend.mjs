// Spend so far: per transcript under ~/.claude/projects/*s2-interactive*, the last cost-state.totalCostUSD (claude's own cumulative figure).
// Read-only. Sessions whose transcript has no cost-state (killed early) count as 0.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const root = path.join(os.homedir(), '.claude/projects');
let total = 0;
let n = 0;
for (const d of fs.readdirSync(root).filter((x) => x.includes('s2-interactive'))) {
  for (const f of fs.readdirSync(path.join(root, d)).filter((x) => x.endsWith('.jsonl'))) {
    let last = 0;
    for (const line of fs.readFileSync(path.join(root, d, f), 'utf8').split('\n')) {
      if (!line.includes('"cost-state"')) continue;
      try {
        const r = JSON.parse(line);
        if (r.type === 'cost-state') last = r.totalCostUSD;
      } catch {}
    }
    total += last;
    n++;
    if (process.argv.includes('-v')) console.log(f.slice(0, 8), d.replace(/.*s2-interactive-?/, ''), last.toFixed(4));
  }
}
console.log(`spend: $${total.toFixed(4)} over ${n} transcripts (cap $2)`);
