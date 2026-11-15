//! Exported `Core` commands: fire-and-forget session calls plus snapshot.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex;

use super::Core;
use super::bridge::RemotePlayer;
use super::bridge::{CoreObserver, PlayerBridge};
use super::pump::pump_loop;
use super::snapshot::build_snapshot;
use super::types::{CoreConfig, CoreError, Snapshot};
use crate::CorePaths;
use crate::api::{ApiClient, AudioQuality, PlayMode};
use crate::cache::CacheMode;
use crate::player::{Player, PlayerState};
use crate::session::Session;

#[uniffi::export]
impl Core {
    #[uniffi::constructor]
    pub fn new(
        config: CoreConfig,
        player_bridge: Arc<dyn PlayerBridge>,
        observer: Arc<dyn CoreObserver>,
    ) -> Result<Arc<Self>, CoreError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| CoreError::Init {
                detail: format!("创建 tokio runtime 失败: {error}"),
            })?;

        let paths = CorePaths {
            data_dir: PathBuf::from(&config.data_dir),
            cache_dir: PathBuf::from(&config.cache_dir),
            cache_mb: config.cache_mb as u64,
            cache_mode: config.cache_mode,
        };
        let api = Arc::new(ApiClient::new(paths).map_err(|error| CoreError::Init {
            detail: format!("{error:#}"),
        })?);

        let remote = RemotePlayer::new(player_bridge);
        let state_tx = remote.state_tx.clone();
        let (session, msg_rx) = runtime.block_on(async {
            let player: Arc<dyn Player> = remote;
            Session::new(api.clone(), player).await
        });
        let session = Arc::new(Mutex::new(session));
        runtime.spawn(pump_loop(session.clone(), msg_rx, observer));

        Ok(Arc::new(Self {
            runtime,
            session,
            api,
            state_tx,
        }))
    }

    // -- commands (fire-and-forget; results arrive as CoreEvents) ----------

    pub fn search(&self, keyword: String) {
        self.run(move |session| async move { session.lock().await.search(keyword) });
    }

    /// Fetch the next page of the current search, if there is one. Safe to
    /// call repeatedly while scrolling; concurrent calls are ignored.
    pub fn load_more_search(&self) {
        self.run(move |session| async move { session.lock().await.load_more_search() });
    }

    pub fn play(&self, index: u32) {
        self.run(move |session| async move {
            let mut session = session.lock().await;
            if let Some(request) = session.app.play_at(index as usize) {
                session.play(request);
            }
        });
    }

    /// Play a cached track by BVID; the cached list becomes the queue.
    pub fn play_cached(&self, bvid: String) {
        self.run(move |session| async move { session.lock().await.play_cached(bvid) });
    }

    pub fn toggle_pause(&self) {
        self.run(move |session| async move { session.lock().await.pause().await });
    }

    pub fn seek(&self, delta: f64) {
        self.run(move |session| async move { session.lock().await.seek(delta).await });
    }

    pub fn seek_to(&self, seconds: f64) {
        self.run(move |session| async move { session.lock().await.seek_to(seconds).await });
    }

    pub fn set_volume(&self, volume: f64) {
        self.run(move |session| async move { session.lock().await.volume(volume).await });
    }

    /// Change the audio track preference (Auto/Aac/Dolby/HiRes). Persisted
    /// across restarts; takes effect on the next track load.
    pub fn set_audio_quality(&self, quality: AudioQuality) {
        self.run(move |session| async move {
            session.lock().await.set_audio_quality(quality);
        });
    }

    /// Change the playback order. Persisted across restarts.
    pub fn set_play_mode(&self, mode: PlayMode) {
        self.run(move |session| async move {
            session.lock().await.set_play_mode(mode);
        });
    }

    pub fn next_track(&self) {
        self.run(move |session| async move { session.lock().await.step_track(1) });
    }

    pub fn previous_track(&self) {
        self.run(move |session| async move { session.lock().await.step_track(-1) });
    }

    /// Play a specific 分P of the current multi-page video (page picker).
    pub fn play_page(&self, page: u32) {
        self.run(move |session| async move { session.lock().await.play_page(page as usize) });
    }

    pub fn retry_current(&self) {
        self.run(move |session| async move { session.lock().await.retry_current() });
    }

    /// Drop the cached lyrics for the current track and fetch them again.
    pub fn refresh_lyrics(&self) {
        self.run(|session| async move { session.lock().await.refresh_lyrics() });
    }

    /// Change the on-disk cache size cap in MiB; 0 disables caching.
    /// Lowering it evicts least recently used files immediately.
    pub fn set_cache_limit_mb(&self, limit_mb: u32) {
        self.run(move |session| async move {
            session.lock().await.set_cache_limit_mb(limit_mb as u64);
        });
    }

    /// Switch what the cache stores (audio included, small files only, off).
    /// Existing files are kept; the UI offers a separate clear action.
    pub fn set_cache_mode(&self, mode: CacheMode) {
        self.run(move |session| async move {
            session.lock().await.set_cache_mode(mode);
        });
    }

    /// Recompute the on-disk cache usage shown in `Snapshot::cache_usage`.
    pub fn refresh_cache_stats(&self) {
        self.run(|session| async move { session.lock().await.refresh_cache_stats() });
    }

    /// Delete cached audio, covers, lyrics and video metadata. The file of
    /// the track currently playing from cache is kept.
    pub fn clear_cache(&self) {
        self.run(|session| async move { session.lock().await.clear_cache() });
    }

    pub fn start_login(&self) {
        self.run(move |session| async move { session.lock().await.open_login() });
    }

    pub fn cancel_login(&self) {
        self.run(|session| async move { session.lock().await.cancel_login() });
    }

    /// Cookie-based login (in-app WebView or pasted text): persists the
    /// cookie, refreshes the account and loads the playlist library.
    pub fn set_cookie(&self, cookie: String) {
        self.run(move |session| async move { session.lock().await.set_cookie_text(cookie).await });
    }

    /// Sign out (for switching accounts).
    pub fn logout(&self) {
        self.run(|session| async move { session.lock().await.logout().await });
    }

    // -- playlist (favorite folder) commands --------------------------------

    /// Show the playlist library and refresh the folder list.
    pub fn open_library(&self) {
        self.run(move |session| async move { session.lock().await.show_library().await });
    }

    /// Return from the song list to the library.
    pub fn open_queue(&self) {
        self.run(move |session| async move {
            session.lock().await.show_queue();
        });
    }

    pub fn refresh_playlists(&self) {
        self.run(move |session| async move {
            session.lock().await.refresh_playlists();
        });
    }

    /// Open a folder loaded in the current snapshot.
    pub fn open_playlist(&self, media_id: i64) {
        self.run(move |session| async move {
            let mut session = session.lock().await;
            let folder = session
                .app
                .playlists
                .iter()
                .find(|folder| folder.id == media_id)
                .cloned();
            if let Some(folder) = folder {
                session.open_playlist(folder);
            } else {
                session.app.set_status("歌单不存在，请刷新列表", true);
            }
        });
    }

    /// Import another user's public favorite folder as a read-only queue.
    /// `input` is a media id, a full folder link, or a b23.tv short link.
    pub fn import_public_playlist(&self, input: String) {
        self.run(move |session| async move {
            session.lock().await.import_public_playlist(input);
        });
    }

    pub fn create_playlist(&self, title: String) {
        self.run(move |session| async move { session.lock().await.create_playlist(title) });
    }

    /// Cancel the BV import that was waiting for a new playlist.
    pub fn cancel_pending_import(&self) {
        self.run(|session| async move { session.lock().await.cancel_pending_import() });
    }

    pub fn rename_playlist(&self, media_id: i64, title: String) {
        self.run(
            move |session| async move { session.lock().await.rename_playlist(media_id, title) },
        );
    }

    pub fn delete_playlist(&self, media_id: i64) {
        self.run(move |session| async move { session.lock().await.delete_playlist(media_id) });
    }

    /// Hide or unhide a favorite folder. The hidden list is persisted per
    /// account; every folder can be hidden.
    pub fn set_playlist_hidden(&self, media_id: i64, hidden: bool) {
        self.run(move |session| async move {
            session.lock().await.set_playlist_hidden(media_id, hidden);
        });
    }

    /// Reveal or collapse hidden folders in the library.
    pub fn toggle_show_hidden(&self) {
        self.run(move |session| async move {
            session.lock().await.toggle_show_hidden();
        });
    }

    /// Add the track identified by `bvid` to a folder.
    pub fn add_to_playlist(&self, media_id: i64, bvid: String) {
        self.run(move |session| async move {
            session.lock().await.add_track_to_playlist(media_id, bvid)
        });
    }

    /// Remove the track identified by `bvid` from a folder.
    pub fn remove_from_playlist(&self, media_id: i64, bvid: String) {
        self.run(move |session| async move {
            session
                .lock()
                .await
                .remove_track_from_playlist(media_id, bvid)
        });
    }

    /// Import a BV id / link into the queue. With `pick_playlist` the
    /// frontend is expected to ask which favorite folder to add it to (the
    /// core parks the track for that); otherwise an open playlist receives
    /// it directly (TUI keyboard flow).
    pub fn import_bv(&self, url: String, pick_playlist: bool) {
        self.run(move |session| async move {
            session.lock().await.import_bv(url, !pick_playlist);
        });
    }

    pub fn stop(&self) {
        self.run(move |session| async move { session.lock().await.stop().await });
    }

    // -- state pushes from the Kotlin player --------------------------------

    /// Kotlin calls this whenever ExoPlayer reports a state change. Feeds the
    /// same `App::on_player_state` logic (progress, EOF auto-advance).
    pub fn report_player_state(&self, state: PlayerState) {
        self.state_tx.send_replace(state);
    }

    // -- images --------------------------------------------------------------

    /// Download one cover thumbnail and return the encoded image bytes.
    /// Kotlin calls this from a background dispatcher so list rows reuse the
    /// core's CDN probing, retry and on-disk cover cache; playback covers
    /// already depend on that path to survive flaky CDN nodes.
    pub fn fetch_thumbnail(&self, url: String) -> Option<Vec<u8>> {
        self.runtime
            .block_on(async { self.api.image_bytes(&url).await.ok() })
    }

    // -- state reads ---------------------------------------------------------

    /// Read the full UI state. Call after each `StateChanged` event.
    pub fn snapshot(&self) -> Snapshot {
        self.runtime.block_on(async {
            let session = self.session.lock().await;
            build_snapshot(&session.app)
        })
    }
}
