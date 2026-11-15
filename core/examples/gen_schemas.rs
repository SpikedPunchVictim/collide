//! Writes the JSON Schemas: the real registry to `schemas/` at the repo root, and the test fixtures' registry to
//! `core/tests/fixtures/schemas/`. This is the Rust half of `pnpm gen`; run that instead of running this.

#[allow(dead_code)]
#[path = "../tests/fixtures/mod.rs"]
mod fixtures;

use collide_core::schema::{self, Registry};
use std::fs;
use std::path::Path;

/// Replaces the `.json` files in `dir` with the registry's schemas.
fn write_schemas(registry: &Registry, dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "json") {
            fs::remove_file(path).unwrap();
        }
    }
    for (name, text) in registry.render() {
        fs::write(dir.join(name), text).unwrap();
    }
}

fn main() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    write_schemas(&schema::registry(), &core.join("../schemas"));
    write_schemas(&fixtures::registry(), &core.join("tests/fixtures/schemas"));
}
