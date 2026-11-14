//! The session loop: owns the app state, dispatches background-task results
//! into it and drives auto-advance.
//!
//! Frontends feed it user commands (search/play/pause/...) and pump events
//! out. Every state mutation goes through `App::on_*` exactly like the
//! original TUI loop did, so both frontends observe identical behaviour.

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::api::ApiClient;
use crate::app::{App, PlayRequest};
use crate::player::Player;
use tasks::cover::spawn_cover;
use tasks::login::spawn_user_info;

mod commands;
mod lifecycle;
mod msg;
mod parse;
mod process;
mod tasks;
#[cfg(test)]
mod tests;

pub use msg::{CoreEvent, Msg};
pub use parse::parse_bvid;

/// How far playback may advance before the position is written to disk again.
const PROGRESS_SAVE_INTERVAL_SECS: f64 = 5.0;

pub struct Session {
    pub api: Arc<ApiClient>,
    pub player: Arc<dyn Player>,
    pub app: App,
    tx: mpsc::UnboundedSender<Msg>,
    /// Bumped for every lyric request; older responses are dropped so a slow
    /// fetch cannot overwrite a newer one (e.g. double-tapped refresh).
    lyrics_seq: u64,
    /// Track restored in a paused session. The first explicit play starts it
    /// at this position instead of from the beginning.
    pending_resume: Option<PendingResume>,
    /// Last values written to `playback.json`, used to throttle position
    /// saves while the player reports progress every 500 ms.
    saved_position: f64,
    saved_paused: bool,
    saved_once: bool,
}

struct PendingResume {
    bvid: String,
    page: usize,
    position: f64,
}

impl Session {
    pub async fn new(
        api: Arc<ApiClient>,
        player: Arc<dyn Player>,
    ) -> (Self, mpsc::UnboundedReceiver<Msg>) {
        let (tx, rx) = mpsc::unbounded_channel();

        // Forward backend state snapshots into the message loop. The mpv
        // backend polls every 500 ms; ExoPlayer pushes on every change.
        let mut state_rx = player.watch();
        let relay_tx = tx.clone();
        tokio::spawn(async move {
            while state_rx.changed().await.is_ok() {
                let state = state_rx.borrow().clone();
                if relay_tx.send(Msg::PlayerState(state)).is_err() {
                    break;
                }
            }
        });

        let mut session = Self {
            api: api.clone(),
            player,
            app: App::new(),
            tx,
            lyrics_seq: 0,
            pending_resume: None,
            saved_position: 0.0,
            saved_paused: false,
            saved_once: false,
        };
        session.app.quality_pref = api.audio_quality();
        session.app.play_mode = api.play_mode();
        session.app.cache_mode = api.cache().mode();
        session.update_cache_stats();
        session.refresh_cached_tracks();
        session.restore_last_playback();
        spawn_user_info(&session.api, session.tx.clone());
        session.app.has_cookie = api.has_cookie().await;
        if session.app.has_cookie {
            // Show the cached library immediately; the network refresh below
            // replaces it when available. This keeps cached audio reachable
            // even when the app starts offline.
            if let Some(folders) = api.cached_favorite_folders().await {
                let hidden = api.hidden_playlists().await;
                session.app.load_cached_playlists(folders, hidden);
            }
            // Keep a restored queue/playlist visible; only fall back to the
            // folder list when there was nothing to restore.
            if session.app.results.is_empty() && session.app.now_playing.is_none() {
                session.app.open_library();
            }
            session.refresh_playlists();
        }
        (session, rx)
    }

    /// Load the last queue and track. Playback stays paused on startup; an
    /// explicit play or retry resumes from the saved position.
    fn restore_last_playback(&mut self) {
        let Some(saved) = crate::resume::load(self.api.data_dir()) else {
            return;
        };
        let request = saved.apply(&mut self.app);
        self.saved_position = self.app.position;
        self.saved_paused = self.app.paused;
        self.saved_once = true;

        // Push the paused track's cover through the same CoverReady event so
        // the UI shows it right away; `image_bytes` serves a cached copy from
        // disk when one exists.
        if let Some(item) = self.app.now_playing.clone()
            && !self.app.cover_key.is_empty()
        {
            spawn_cover(
                &self.api,
                self.tx.clone(),
                self.app.cover_key.clone(),
                item.pic,
            );
        }

        if let Some(request) = request {
            // Kept until playback actually starts so the play button and the
            // retry action resume from the same position.
            self.pending_resume = Some(PendingResume {
                bvid: request.item.bvid.clone(),
                page: request.page_index,
                position: request.start_at,
            });
        }
    }

    // ----- commands -------------------------------------------------------

    pub(in crate::session) fn pending_resume_request(&self) -> Option<PlayRequest> {
        if self.app.has_media || self.app.pending_play {
            return None;
        }
        let pending = self.pending_resume.as_ref()?;
        let item = self.app.now_playing.clone()?;
        if item.bvid != pending.bvid || self.app.page != pending.page {
            return None;
        }
        Some(PlayRequest {
            item,
            page_index: pending.page,
            start_at: pending.position,
        })
    }

    fn progress_save_due(&self) -> bool {
        if !self.saved_once {
            return true;
        }
        (self.app.position - self.saved_position).abs() >= PROGRESS_SAVE_INTERVAL_SECS
            || self.saved_paused != self.app.paused
    }
}
