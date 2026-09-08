fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=GDOM_UPDATER_PUBLIC_KEY");
    println!("cargo:rerun-if-env-changed=GDOM_DEFAULT_CLIENT_SECRET");
    let attributes = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    tauri_build::try_build(attributes)?;
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Link once for every executable: Tauri's default resource only covers app binaries.
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
    Ok(())
}
