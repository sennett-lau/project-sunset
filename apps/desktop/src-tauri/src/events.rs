use serde::{Deserialize, Serialize};
use specta::Type;
use sunset_core::Sunset;
use sunset_core::domain::AppEvent;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::sync::broadcast::error::RecvError;

/// The one Tauri event: every core event, unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct CoreEvent(pub AppEvent);

/// Forward core events to the webview until the app exits.
pub fn forward(app: AppHandle, sunset: &Sunset) {
    let mut rx = sunset.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Err(e) = CoreEvent(event).emit(&app) {
                        tracing::warn!(error = %e, "could not emit event");
                    }
                }
                // The UI refetches on the next event; a few lost deltas are acceptable.
                Err(RecvError::Lagged(n)) => tracing::warn!(skipped = n, "event forwarder lagged"),
                Err(RecvError::Closed) => break,
            }
        }
    });
}
