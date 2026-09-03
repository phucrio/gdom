mod application;
mod commands;
pub mod domain;
mod infrastructure;
mod runtime;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = tauri::Builder::default().run(tauri::generate_context!()) {
        eprintln!("failed to run GDOM: {error}");
    }
}
