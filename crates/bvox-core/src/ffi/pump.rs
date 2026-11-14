//! Session message pump: forwards internal messages to the observer.

use std::sync::Arc;

use tokio::sync::Mutex;

use super::bridge::CoreObserver;
use super::types::CoreEvent;
use crate::session::{CoreEvent as SessionEvent, Msg, Session};

/// messages into observer events.
pub(super) async fn pump_loop(
    session: Arc<Mutex<Session>>,
    mut msg_rx: tokio::sync::mpsc::UnboundedReceiver<Msg>,
    observer: Arc<dyn CoreObserver>,
) {
    while let Some(message) = msg_rx.recv().await {
        let event = session.lock().await.process(message);
        match event {
            SessionEvent::StateChanged => observer.on_event(CoreEvent::StateChanged),
            SessionEvent::CoverReady { key, bytes } => {
                observer.on_event(CoreEvent::CoverReady { key, bytes });
            }
            SessionEvent::Closed => break,
        }
    }
}
