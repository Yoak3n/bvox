//! Frontend-agnostic playback interface.
//!
//! The core drives playback through this trait only. The desktop TUI supplies
//! the mpv implementation (`bili-music-tui::mpv`); the Android app bridges the
//! calls to ExoPlayer over UniFFI. Playback state flows back through a
//! `watch` channel so the session loop treats both backends identically.

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::watch;

/// Point-in-time playback snapshot. The desktop backend polls mpv for this;
/// the Android backend receives it from ExoPlayer callbacks.
#[derive(Clone, Debug, Default, PartialEq, uniffi::Record)]
pub struct PlayerState {
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
    pub volume: f64,
    pub eof: bool,
    pub idle: bool,
}

/// Stream hints the player needs for remote audio. Bilibili CDNs reject
/// requests without a browser-like user agent and the right referer.
#[derive(Clone, Debug)]
pub struct PlayerConfig {
    pub user_agent: String,
    pub referer: String,
}

#[async_trait]
pub trait Player: Send + Sync {
    /// True once a player backend exists and can accept commands.
    async fn is_started(&self) -> bool;

    /// Start playing `target` (a remote URL or a local cached file path),
    /// replacing whatever is currently loaded.
    async fn load(&self, target: &str, config: &PlayerConfig) -> Result<()>;

    /// Flip pause state, returning the new paused flag.
    async fn toggle_pause(&self) -> Result<bool>;

    /// Seek relative to the current position (may be negative).
    async fn seek(&self, seconds: f64) -> Result<()>;

    /// Seek to an absolute position in seconds.
    async fn seek_to(&self, seconds: f64) -> Result<()>;

    /// Set the output volume (0-150, 100 = default).
    async fn set_volume(&self, volume: f64) -> Result<()>;

    /// Stop playback and release the backend.
    async fn quit(&self);

    /// Subscribe to state snapshots; the backend decides the update cadence.
    fn watch(&self) -> watch::Receiver<PlayerState>;
}
