use std::fs;
use std::path::Path;

/// Gives the owner write permission on everything under `path` (directories first, so they can be read), so the tree
/// can be deleted.
#[cfg(unix)]
fn make_writable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    if meta.file_type().is_symlink() {
        return;
    }
    let _ = fs::set_permissions(
        path,
        fs::Permissions::from_mode(meta.permissions().mode() | 0o200),
    );
    if meta.is_dir() {
        for entry in fs::read_dir(path).into_iter().flatten().flatten() {
            make_writable(&entry.path());
        }
    }
}

#[cfg(not(unix))]
fn make_writable(_path: &Path) {}

fn main() {
    // The window embeds the built front end. Create the folder if `pnpm build` has not run yet,
    // so that `cargo build` works on a fresh clone.
    std::fs::create_dir_all("../dist").expect("create app/dist");

    // Tauri refuses a `bundle.resources` path that does not exist, so a fresh clone needs the folder that
    // `pnpm build:packs` fills with the built-in packs. A release build ships what is in it, so it must not be empty.
    let packs = "resources/builtin-packs";
    std::fs::create_dir_all(packs).expect("create the built-in packs folder");
    println!("cargo:rerun-if-changed={packs}");
    let empty = std::fs::read_dir(packs)
        .expect("read the built-in packs folder")
        .next()
        .is_none();
    if empty && std::env::var("PROFILE").as_deref() == Ok("release") {
        panic!("{packs} is empty: run `pnpm build:packs` before a release build");
    }

    // Tauri copies the resources next to the binaries (`target/<profile>/builtin-packs`), keeping the staged files'
    // read-only modes, and then cannot overwrite that copy on the next build. Remove the old copy first; Tauri
    // makes a fresh one. OUT_DIR is `target/<profile>/build/<package>-<hash>/out`.
    let out_dir = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR");
    if let Some(profile_dir) = Path::new(&out_dir).ancestors().nth(3) {
        let copy = profile_dir.join("builtin-packs");
        make_writable(&copy);
        let _ = fs::remove_dir_all(&copy);
    }

    // The app's own command is named in an app manifest, so a capability has to grant it (`allow-rpc`). Without a
    // manifest Tauri lets every window call every command of the app. `capabilities/main.json` grants `rpc` and the
    // event channel's listen and unlisten, and nothing else.
    let manifest = tauri_build::AppManifest::new().commands(&["rpc"]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("tauri_build failed: check tauri.conf.json and the capability files");
}
