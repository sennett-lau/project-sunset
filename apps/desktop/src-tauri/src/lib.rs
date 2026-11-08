//! Tauri shell. Keep this thin: build the core, register commands, forward events.

use sunset_core::{AppIdentity, Sunset};
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // productName and identifier come from tauri.conf.json, which `pnpm identity:sync`
            // fills from app-identity.json. Never hard-code them (docs/architecture.md §2.7).
            let identity = AppIdentity::new(
                app.package_info().name.clone(),
                app.config().identifier.clone(),
            )?;
            let data_dir = app.path().app_data_dir()?;
            app.manage(Sunset::new(identity, data_dir));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start the app");
}
