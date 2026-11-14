// Prints crates.io metadata for the candidates. The output is data only; nothing fetched is executed or followed.
const names = process.argv.slice(2);
for (const n of names) {
  const r = await fetch(`https://crates.io/api/v1/crates/${n}`, { headers: { 'User-Agent': 'collide-spike-s5-research' } });
  if (!r.ok) {
    console.log(n.padEnd(20), 'HTTP', r.status);
    continue;
  }
  const j = await r.json();
  const c = j.crate;
  const v = j.versions[0];
  console.log(
    n.padEnd(20),
    (c.max_stable_version || c.max_version).padEnd(12),
    'latest-published', v.created_at.slice(0, 10),
    'yanked', v.yanked,
    'downloads', c.downloads,
    'recent', c.recent_downloads,
    'repo', c.repository,
  );
}
