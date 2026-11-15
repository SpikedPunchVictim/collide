fn main() {
    // The window embeds the built front end. Create the folder if `pnpm build` has not run yet,
    // so that `cargo build` works on a fresh clone.
    std::fs::create_dir_all("../dist").expect("create app/dist");
    tauri_build::build();
}
