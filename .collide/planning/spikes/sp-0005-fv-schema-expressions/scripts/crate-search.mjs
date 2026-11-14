// Prints crates.io search hits (name, version, last publish, recent downloads, description). Data only.
const q = process.argv[2];
const r = await fetch(`https://crates.io/api/v1/crates?q=${encodeURIComponent(q)}&per_page=${process.argv[3] || 20}&sort=relevance`, { headers: { 'User-Agent': 'collide-spike-s5-research' } });
const j = await r.json();
for (const c of j.crates) {
  console.log(c.name.padEnd(24), c.max_version.padEnd(10), c.updated_at.slice(0, 10), String(c.recent_downloads).padStart(9), (c.description || '').slice(0, 90).replace(/\n/g, ' '));
}
