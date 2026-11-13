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

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::{Mutex, watch};

use crate::CorePaths;
use crate::api::{ApiClient, AudioQuality, FavoriteFolder, LyricLine, PlayMode, VideoItem};
use crate::app::{App, LoginModal};
use crate::cache::{CacheMode, CachedTrack};
use crate::player::{Player, PlayerConfig, PlayerState};
use crate::session::{CoreEvent as SessionEvent, Msg, Session};

// ----- exported data types ------------------------------------------------

/// Filesystem/config knobs for one app instance.
#[derive(uniffi::Record, Clone)]
pub struct CoreConfig {
    pub data_dir: String,
    pub cache_dir: String,
    /// Cache size cap in MiB; 0 disables caching.
    pub cache_mb: u32,
    /// Initial cache strategy; the UI can change it at runtime.
    pub cache_mode: CacheMode,
}

/// Full UI state, read after every `StateChanged` event.
#[derive(uniffi::Record, Clone)]
pub struct Snapshot {
    pub queue_title: String,
    pub results: Vec<VideoItem>,
    pub selected: Option<u32>,
    pub current: Option<u32>,
    pub status: String,
    pub error: bool,
    pub loading: bool,
    /// True when the current search has another page to fetch.
    pub has_more_results: bool,
    /// True while the next search page is in flight.
    pub loading_more: bool,
    /// Total hits reported by Bilibili for the current search (0 otherwise).
    pub search_total: u64,
    pub now_playing: Option<VideoItem>,
    pub quality: String,
    pub page: u32,
    pub page_count: u32,
    pub part: String,
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
    /// 0-150 scale, 100 is the default.
    pub volume: f64,
    pub has_media: bool,
    pub pending_play: bool,
    /// True when a cookie is stored; distinguishes offline starts from a
    /// logged-out account.
    pub has_cookie: bool,
    pub user: Option<String>,
    /// Avatar URL for the logged-in account, when available.
    pub user_face: Option<String>,
    /// True when the logged-in account has an active 大会员 subscription.
    pub vip: bool,
    /// Audio track preference persisted by the core.
    pub quality_pref: AudioQuality,
    /// Playback order (sequential / repeat-all / repeat-one / shuffle).
    pub play_mode: PlayMode,
    pub login: Option<LoginModal>,
    /// True when the main panel shows favorite folders instead of songs.
    pub in_library: bool,
    pub playlists: Vec<FavoriteFolder>,
    pub playlists_loading: bool,
    /// Folder ids hidden by the user; persisted per account.
    pub hidden_playlist_ids: Vec<i64>,
    /// True while hidden folders are temporarily revealed in the library.
    pub show_hidden: bool,
    /// Title of a BV import waiting for the user to pick a playlist.
    pub pending_import_title: Option<String>,
    /// BVID of the parked import, for the add-to-playlist picker.
    pub pending_import_bvid: Option<String>,
    pub current_playlist: Option<FavoriteFolder>,
    pub show_lyrics: bool,
    pub lyrics: Vec<LyricLine>,
    pub lyrics_status: String,
    /// `bvid#cid#page` of the track covers belong to; correlate with
    /// `CoreEvent::CoverReady.key`.
    pub cover_key: String,
    pub cache_progress: Option<CacheProgress>,
    pub cached_now: bool,
    /// Cache strategy currently in effect.
    pub cache_mode: CacheMode,
    /// BVIDs with completed audio files, for cached markers in list views.
    pub cached_bvids: Vec<String>,
    /// Playable offline entries for the "cached songs" screen, newest first.
    pub cached_tracks: Vec<CachedTrack>,
    /// On-disk cache usage; `None` until the first session refresh.
    pub cache_usage: Option<CacheUsage>,
}

#[derive(uniffi::Record, Clone)]
pub struct CacheProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// Disk space used by the cache, split by category.
#[derive(uniffi::Record, Clone)]
pub struct CacheUsage {
    pub used_bytes: u64,
    /// Configured cap; 0 when caching is disabled.
    pub limit_bytes: u64,
    pub audio_bytes: u64,
    pub cover_bytes: u64,
    pub lyrics_bytes: u64,
    pub view_bytes: u64,
    pub file_count: u64,
}

#[derive(uniffi::Enum, Clone, Debug)]
pub enum CoreEvent {
    /// Something in the session state changed; re-read `Core::snapshot`.
    StateChanged,
    /// Cover image bytes for the current track (`key` = `bvid#cid#page`).
    CoverReady { key: String, bytes: Vec<u8> },
}

#[derive(Debug, uniffi::Error)]
pub enum CoreError {
    Init { detail: String },
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Init { detail } => write!(f, "{detail}"),
        }
    }
}

// ----- callback interfaces --------------------------------------------------

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
struct RemotePlayer {
    bridge: Arc<dyn PlayerBridge>,
    state_tx: watch::Sender<PlayerState>,
}

impl RemotePlayer {
    fn new(bridge: Arc<dyn PlayerBridge>) -> Arc<Self> {
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
            Session::new(api, player).await
        });
        let session = Arc::new(Mutex::new(session));
        runtime.spawn(pump_loop(session.clone(), msg_rx, observer));

        Ok(Arc::new(Self {
            runtime,
            session,
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

    pub fn import_bv(&self, url: String) {
        self.run(move |session| async move { session.lock().await.import_bv(url) });
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

    // -- state reads ---------------------------------------------------------

    /// Read the full UI state. Call after each `StateChanged` event.
    pub fn snapshot(&self) -> Snapshot {
        self.runtime.block_on(async {
            let session = self.session.lock().await;
            build_snapshot(&session.app)
        })
    }
}

fn build_snapshot(app: &App) -> Snapshot {
    Snapshot {
        queue_title: app.results_title.clone(),
        results: app.results.clone(),
        selected: app.selected.map(|value| value as u32),
        current: app.current.map(|value| value as u32),
        status: app.status.clone(),
        error: app.error,
        loading: app.loading,
        has_more_results: app.has_more_results(),
        loading_more: app.loading_more,
        search_total: app.search_total,
        now_playing: app.now_playing.clone(),
        quality: app.quality.clone(),
        page: app.page as u32,
        page_count: app.page_count as u32,
        part: app.part.clone(),
        position: app.position,
        duration: app.duration,
        paused: app.paused,
        volume: app.volume,
        has_media: app.has_media,
        pending_play: app.pending_play,
        has_cookie: app.has_cookie,
        user: app.user.clone(),
        user_face: app.user_face.clone(),
        vip: app.vip,
        quality_pref: app.quality_pref,
        play_mode: app.play_mode,
        login: app.login.clone(),
        in_library: app.view == crate::app::MainView::Library,
        playlists: app.playlists.clone(),
        playlists_loading: app.playlists_loading,
        hidden_playlist_ids: {
            let mut ids = app.hidden_playlists.iter().copied().collect::<Vec<_>>();
            ids.sort_unstable();
            ids
        },
        show_hidden: app.show_hidden,
        pending_import_title: app
            .pending_import
            .as_ref()
            .map(|pending| pending.title.clone()),
        pending_import_bvid: app
            .pending_import
            .as_ref()
            .map(|pending| pending.bvid.clone()),
        current_playlist: app.current_playlist.clone(),
        show_lyrics: app.show_lyrics,
        lyrics: app.lyrics.clone(),
        lyrics_status: app.lyrics_status.clone(),
        cover_key: app.cover_key.clone(),
        cache_progress: app
            .cache_progress
            .map(|(downloaded, total)| CacheProgress { downloaded, total }),
        cached_now: app.cached_now,
        cache_mode: app.cache_mode,
        cached_bvids: {
            let mut bvids = app.cached_bvids.iter().cloned().collect::<Vec<_>>();
            bvids.sort();
            bvids
        },
        cached_tracks: app.cached_tracks.clone(),
        cache_usage: app.cache_stats.map(|stats| CacheUsage {
            used_bytes: stats.total_bytes,
            limit_bytes: stats.limit_bytes,
            audio_bytes: stats.audio_bytes,
            cover_bytes: stats.cover_bytes,
            lyrics_bytes: stats.lyrics_bytes,
            view_bytes: stats.view_bytes,
            file_count: stats.file_count,
        }),
    }
}

/// Drive the session message loop on the runtime, translating internal
/// messages into observer events.
async fn pump_loop(
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
