//! Tauri shell. Keep this thin: build the core, register commands, forward events.
//! All logic lives in `sunset-core`.

mod commands;
mod events;

use std::time::Duration;

use sunset_core::{AppIdentity, Sunset, SunsetConfig, mock};
use tauri::Manager;
use tauri_specta::{Builder, ErrorHandlingMode, collect_commands, collect_events};

/// Every command and event the webview can use. The TypeScript bindings are generated from this.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .error_handling(ErrorHandlingMode::Throw)
        // One TypeScript type per Rust type; our types serialize and deserialize the same way.
        .disable_serde_phases()
        .commands(collect_commands![
            commands::send_message,
            commands::list_turns,
            commands::pending_proposals,
            commands::recent_proposals,
            commands::approve_proposal,
            commands::reject_proposal,
            commands::undo_proposal,
            commands::list_concepts,
            commands::concept_page,
            commands::graph,
            commands::save_mode,
            commands::set_save_mode,
        ])
        .events(collect_events![events::CoreEvent])
}

pub fn run() {
    let builder = specta_builder();
    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            // productName and identifier come from tauri.conf.json, which `pnpm identity:sync`
            // fills from app-identity.json. Never hard-code them (docs/architecture.md §2.7).
            let identity = AppIdentity::new(
                app.package_info().name.clone(),
                app.config().identifier.clone(),
            )?;
            let config = SunsetConfig {
                identity,
                data_dir: app.path().app_data_dir()?,
                // Phase 0: scripted models. Phase 1 reads provider settings instead.
                providers: mock::providers(Duration::from_millis(25)),
            };
            let sunset = tauri::async_runtime::block_on(Sunset::open(config))?;
            events::forward(app.handle().clone(), &sunset);
            app.manage(sunset);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start the app");
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use specta_typescript::Typescript;

    const HEADER: &str =
        "// Generated from apps/desktop/src-tauri by `pnpm gen:bindings`. Do not edit.\n";

    fn bindings_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../packages/bindings/src/index.ts")
    }

    /// Fails when the checked-in bindings are stale. `SUNSET_WRITE_BINDINGS=1` rewrites them.
    #[test]
    fn typescript_bindings_are_up_to_date() {
        let tmp = std::env::temp_dir().join(format!("sunset-bindings-{}.ts", std::process::id()));
        super::specta_builder()
            .export(Typescript::default().header(HEADER), &tmp)
            .expect("export bindings");
        let fresh = std::fs::read_to_string(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        let path = bindings_path();
        if std::env::var_os("SUNSET_WRITE_BINDINGS").is_some() {
            std::fs::write(&path, &fresh).unwrap();
            return;
        }
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            current == fresh,
            "packages/bindings/src/index.ts is out of date. Run: pnpm gen:bindings"
        );
    }
}
