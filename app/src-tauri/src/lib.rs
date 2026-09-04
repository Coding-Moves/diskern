//! Tauri shell. Thin: commands validate input, call diskern-core, return JSON.
//! The webview can ONLY do what these commands expose — capability-scoped.

mod commands;

pub fn run() {
    tauri::Builder::default()
        .manage(commands::ActiveScan::default())
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
            commands::quarantine_finding,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Diskern");
}
