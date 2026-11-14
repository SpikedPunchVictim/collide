//! A synthetic corpus shaped like the real Orchard one (read-only census of the Orchard repo, 1599 task files):
//! status mix done 829 / specified 593 / dropped 141 / in_review 18 / ready 13 / doing 5, and `depends_on` lengths
//! 0:1139 1:290 2:78 3:29 4:18 5:14 6:9 7:10 8:2 9:2 10:2 11:2 14:2 15:1 24:1. Deterministic (LCG), no data copied.

use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct Task {
    pub key: String,
    pub id: String,
    pub source: String,
    pub title: String,
    pub status: String,
    pub category: String,
    pub kind: String,
    pub labels: Vec<String>,
    pub collections: Vec<String>,
    pub depends_on: Vec<String>,
    pub author: String,
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const STATUS: &[(&str, &str, u64)] = &[
    ("done", "done", 829),
    ("specified", "backlog", 593),
    ("dropped", "dropped", 141),
    ("in_review", "review", 18),
    ("ready", "ready", 13),
    ("doing", "active", 5),
];
const DEP_LEN: &[(usize, u64)] = &[
    (0, 1139), (1, 290), (2, 78), (3, 29), (4, 18), (5, 14), (6, 9), (7, 10), (8, 2), (9, 2), (10, 2), (11, 2), (14, 2), (15, 1), (24, 1),
];
const LABELS: &[&str] = &["mvp", "edge", "p26", "infra", "security", "ui", "docs", "perf", "billing", "ingest", "export", "tenant"];
const COLLECTIONS: &[&str] = &["plan:E-07", "plan:E-01", "plan:E-02", "plan:E-03", "plan:E-04"];
const WORDS: &[&str] = &[
    "edge", "ingest", "tenant", "signed", "asset", "outbox", "route", "detector", "rules", "change", "log", "quarantine", "export",
    "source", "drive", "postgres", "verify", "offline", "retry", "schema",
];

fn pick<T: Copy>(rng: &mut Lcg, table: &[(T, u64)]) -> T {
    let total: u64 = table.iter().map(|(_, w)| *w).sum();
    let mut x = rng.below(total);
    for (v, w) in table {
        if x < *w {
            return *v;
        }
        x -= *w;
    }
    table[0].0
}

pub fn corpus(n: usize) -> Vec<Task> {
    let mut rng = Lcg(0x5eed);
    let status_table: Vec<((&str, &str), u64)> = STATUS.iter().map(|(s, c, w)| ((*s, *c), *w)).collect();
    let mut tasks: Vec<Task> = Vec::with_capacity(n);
    for i in 0..n {
        let id = format!("T-{:02}-{:03}", i / 40, i % 40 + 100);
        let (status, category) = pick(&mut rng, &status_table);
        let dep_len = if i == 0 { 0 } else { pick(&mut rng, DEP_LEN).min(i) };
        let depends_on = (0..dep_len).map(|_| tasks[rng.below(i as u64) as usize].id.clone()).collect();
        let nlabels = rng.below(4) as usize;
        let labels = (0..nlabels).map(|_| LABELS[rng.below(LABELS.len() as u64) as usize].to_string()).collect();
        let collections = (0..1 + rng.below(2)).map(|_| COLLECTIONS[rng.below(COLLECTIONS.len() as u64) as usize].to_string()).collect();
        let title: Vec<&str> = (0..10 + rng.below(6)).map(|_| WORDS[rng.below(WORDS.len() as u64) as usize]).collect();
        tasks.push(Task {
            key: format!("plan:{id}"),
            id,
            source: "plan".into(),
            title: title.join(" "),
            status: status.into(),
            category: category.into(),
            kind: ["task", "spike", "bug"][rng.below(3) as usize].into(),
            labels,
            collections,
            depends_on,
            author: ["me", "bot", "dev-a"][rng.below(3) as usize].into(),
        });
    }
    tasks
}
