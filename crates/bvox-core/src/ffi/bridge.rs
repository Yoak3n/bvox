//! Kotlin-side bridges: playback commands in, player state out.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::watch;

use super::types::CoreEvent;
use crate::player::{Player, PlayerConfig, PlayerState};

/// Implemented in Kotlin; the core forwards playback commands to it.
#[uniffi::export(foreign)]
pub trait PlayerBridge: Send + Sync {
    /// Start playing `url` (a remote stream URL or a local cached file path).
    /// `title`/`artist`/`cover_url` describe the track for the platform media
    /// notification and lock screen.
    #[allow(clippy::too_many_arguments)]
    fn load(
        &self,
        url: String,
        user_agent: String,
        referer: String,
        title: String,
        artist: String,
        cover_url: Option<String>,
        // Seconds to start playback from, zero for a normal start.
        start_position: f64,
    );
    fn toggle_pause(&self);
    /// Seek relative to the current position (may be negative).
    fn seek_by(&self, seconds: f64);
    /// Seek to an absolute position in seconds.
    fn seek_to(&self, seconds: f64);
    fn set_volume(&self, volume: f64);
    fn stop(&self);
}

/// Implemented in Kotlin; receives session events. Called on tokio worker
/// threads — dispatch back to the main thread inside the implementation.
#[uniffi::export(foreign)]
pub trait CoreObserver: Send + Sync {
    fn on_event(&self, event: CoreEvent);
}

// ----- Android-side player --------------------------------------------------

/// `Player` implementation that forwards everything to the Kotlin bridge.
/// Playback state flows back via `Core::report_player_state`.
pub(super) struct RemotePlayer {
    pub(super) bridge: Arc<dyn PlayerBridge>,
    pub(super) state_tx: watch::Sender<PlayerState>,
}

impl RemotePlayer {
    pub(super) fn new(bridge: Arc<dyn PlayerBridge>) -> Arc<Self> {
        let (state_tx, _) = watch::channel(PlayerState::default());
        Arc::new(Self { bridge, state_tx })
    }
}

#[async_trait]
impl Player for RemotePlayer {
    async fn is_started(&self) -> bool {
        // ExoPlayer is managed Kotlin-side and always available.
        true
    }

    async fn load(&self, target: &str, config: &PlayerConfig) -> Result<()> {
        self.bridge.load(
            target.to_string(),
            config.user_agent.clone(),
            config.referer.clone(),
            config.title.clone(),
            config.artist.clone(),
            config.cover_url.clone(),
            config.start_position.max(0.0),
        );
        Ok(())
    }

    async fn toggle_pause(&self) -> Result<bool> {
        // The resulting state arrives via report_player_state.
        self.bridge.toggle_pause();
        Ok(false)
    }

    async fn seek(&self, seconds: f64) -> Result<()> {
        self.bridge.seek_by(seconds);
        Ok(())
    }

    async fn seek_to(&self, seconds: f64) -> Result<()> {
        self.bridge.seek_to(seconds);
        Ok(())
    }

    async fn set_volume(&self, volume: f64) -> Result<()> {
        self.bridge.set_volume(volume);
        Ok(())
    }

    async fn quit(&self) {
        self.bridge.stop();
    }

    fn watch(&self) -> watch::Receiver<PlayerState> {
        self.state_tx.subscribe()
    }
}

// ----- stream header constants ----------------------------------------------

// media3 1.11 dropped per-item HTTP headers from RequestMetadata, so the
// player factory needs the values up front. Expose them as free functions
// (callable before any Core exists).
#[uniffi::export]
pub fn player_user_agent() -> String {
    crate::api::UA.to_string()
}

#[uniffi::export]
pub fn player_referer() -> String {
    crate::api::REFERER.to_string()
}
