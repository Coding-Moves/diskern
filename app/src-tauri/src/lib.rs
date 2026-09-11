//! Tauri shell. Thin: commands validate input, call diskern-core, return JSON.
//! The webview can ONLY do what these commands expose — capability-scoped.

mod commands;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    tauri::Builder::default()
        .manage(std::sync::Arc::new(commands::ScanAuthority::default()))
        .setup(|app| {
            // Updater: desktop only, checked from the frontend after launch.
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            commands::start_scan,
            commands::cancel_scan,
            commands::quarantine_finding,
            commands::list_quarantine,
            commands::restore_quarantined,
            commands::purge_quarantine,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Diskern");
}
