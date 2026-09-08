fn main() {
    println!("cargo:rerun-if-env-changed=GDOM_UPDATER_PUBLIC_KEY");
    println!("cargo:rerun-if-env-changed=GDOM_DEFAULT_CLIENT_SECRET");
    tauri_build::build()
}
