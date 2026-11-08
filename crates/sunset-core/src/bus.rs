use sunset_domain::AppEvent;
use tokio::sync::broadcast;

/// Fan-out of [`AppEvent`]s to every subscriber (the desktop shell, tests, later a server).
#[derive(Debug, Clone)]
pub struct EventBus {
    tx: broadcast::Sender<AppEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            tx: broadcast::channel(1024).0,
        }
    }

    pub fn emit(&self, event: AppEvent) {
        // No subscribers is fine: nothing is listening yet.
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.tx.subscribe()
    }
}
