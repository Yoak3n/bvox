//! `Player` trait adapter around the IPC layer.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::watch;

use std::time::Duration;
use tokio::sync::Mutex;

use super::Mpv;
use bvox_core::player::{Player, PlayerConfig, PlayerState};

/// `Player` implementation over the mpv IPC layer.
///
/// Commands are serialized through an inner mutex; state snapshots are pushed
/// to a `watch` channel every 500 ms so `bvox_core::session::Session` sees the
/// same event stream an ExoPlayer-backed player would produce.
pub struct MpvPlayer {
    inner: Mutex<Mpv>,
    state_tx: watch::Sender<PlayerState>,
}

impl MpvPlayer {
    pub fn new() -> Arc<Self> {
        let (state_tx, _) = watch::channel(PlayerState::default());
        let player = Arc::new(Self {
            inner: Mutex::new(Mpv::new()),
            state_tx,
        });

        // Poll mpv for property snapshots. Holds a weak reference so the task
        // ends once the player is dropped.
        let weak = Arc::downgrade(&player);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let Some(player) = weak.upgrade() else {
                    break;
                };
                let state = player.inner.lock().await.poll_state().await;
                if let Ok(Some(state)) = state {
                    player.state_tx.send_replace(state);
                }
            }
        });

        player
    }
}

#[async_trait]
impl Player for MpvPlayer {
    async fn is_started(&self) -> bool {
        self.inner.lock().await.is_started()
    }

    async fn load(&self, target: &str, config: &PlayerConfig) -> Result<()> {
        self.inner.lock().await.load(target, config).await
    }

    async fn toggle_pause(&self) -> Result<bool> {
        self.inner.lock().await.toggle_pause().await
    }

    async fn seek(&self, seconds: f64) -> Result<()> {
        self.inner.lock().await.seek(seconds).await
    }

    async fn seek_to(&self, seconds: f64) -> Result<()> {
        self.inner.lock().await.seek_to(seconds).await
    }

    async fn set_volume(&self, volume: f64) -> Result<()> {
        self.inner.lock().await.set_volume(volume).await
    }

    async fn quit(&self) {
        self.inner.lock().await.quit().await;
    }

    fn watch(&self) -> watch::Receiver<PlayerState> {
        self.state_tx.subscribe()
    }
}
