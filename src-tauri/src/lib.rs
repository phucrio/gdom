pub mod application;
mod commands;
pub mod domain;
pub mod infrastructure;
mod runtime;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "windows")]
    let builder = match infrastructure::secrets::WindowsCredentialStore::new() {
        Ok(store) => builder.manage(store),
        Err(error) => {
            eprintln!("failed to initialize GDOM: {error}");
            return;
        }
    };

    if let Err(error) = builder.run(tauri::generate_context!()) {
        eprintln!("failed to run GDOM: {error}");
    }
}
