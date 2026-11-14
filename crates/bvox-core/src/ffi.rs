//! UniFFI export surface for the Android app.
//!
//! The Kotlin side talks to exactly one object: [`Core`]. Commands are
//! fire-and-forget methods (they spawn onto the internal tokio runtime);
//! results come back as [`CoreEvent`]s through the [`CoreObserver`] callback.
//! UI state is read via [`Core::snapshot`], a plain record mirroring
//! `App`. Playback runs inside ExoPlayer: Kotlin implements
//! [`PlayerBridge`], and ExoPlayer state changes are pushed back through
//! [`Core::report_player_state`], which feeds the same auto-advance logic
//! the desktop uses.

use std::sync::Arc;

use tokio::sync::{Mutex, watch};

use crate::player::PlayerState;
use crate::session::Session;

mod bridge;
mod commands;
mod pump;
mod snapshot;
mod types;

pub use bridge::{CoreObserver, PlayerBridge, player_referer, player_user_agent};
pub use types::{CacheProgress, CacheUsage, CoreConfig, CoreError, CoreEvent, Snapshot};

// ----- the Core object ------------------------------------------------------

#[derive(uniffi::Object)]
pub struct Core {
    runtime: tokio::runtime::Runtime,
    session: Arc<Mutex<Session>>,
    state_tx: watch::Sender<PlayerState>,
}

impl Core {
    /// Spawn a command on the internal runtime with brief session access.
    fn run<F, Fut>(&self, make_future: F)
    where
        F: FnOnce(Arc<Mutex<Session>>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let session = self.session.clone();
        self.runtime.spawn(async move {
            make_future(session).await;
        });
    }
}
