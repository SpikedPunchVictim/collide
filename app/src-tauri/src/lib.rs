pub const NAME: &str = "collide-app";

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running Collide");
}
