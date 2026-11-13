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
use crate::app::{App, LoginModal};
use crate::api::{ApiClient, FavoriteFolder, LyricLine, VideoItem};
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
    pub user: Option<String>,
    pub login: Option<LoginModal>,
    pub favorites: Option<FavoritesState>,
    pub show_lyrics: bool,
    pub lyrics: Vec<LyricLine>,
    pub lyrics_status: String,
    /// `bvid#cid#page` of the track covers belong to; correlate with
    /// `CoreEvent::CoverReady.key`.
    pub cover_key: String,
    pub cache_progress: Option<CacheProgress>,
    pub cached_now: bool,
}

#[derive(uniffi::Record, Clone)]
pub struct FavoritesState {
    pub folders: Vec<FavoriteFolder>,
    pub selected: Option<u32>,
    pub loading: bool,
    pub status: String,
}

#[derive(uniffi::Record, Clone)]
pub struct CacheProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
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
    /// Volume is on the 0-150 scale (100 = default).
    fn load(&self, url: String, user_agent: String, referer: String);
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
        self.bridge
            .load(target.to_string(), config.user_agent.clone(), config.referer.clone());
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

    pub fn play(&self, index: u32) {
        self.run(move |session| async move {
            let mut session = session.lock().await;
            if let Some(request) = session.app.play_at(index as usize) {
                session.play(request);
            }
        });
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

    pub fn next_track(&self) {
        self.run(move |session| async move { session.lock().await.step_track(1) });
    }

    pub fn previous_track(&self) {
        self.run(move |session| async move { session.lock().await.step_track(-1) });
    }

    pub fn retry_current(&self) {
        self.run(move |session| async move { session.lock().await.retry_current() });
    }

    pub fn start_login(&self) {
        self.run(move |session| async move { session.lock().await.open_login() });
    }

    pub fn cancel_login(&self) {
        self.run(|session| async move { session.lock().await.cancel_login() });
    }

    pub fn close_favorites(&self) {
        self.run(|session| async move { session.lock().await.close_favorites() });
    }

    /// Cookie-based login (in-app WebView or pasted text): persists the
    /// cookie, refreshes the account and pulls the cloud-sync folder.
    pub fn set_cookie(&self, cookie: String) {
        self.run(move |session| async move {
            session.lock().await.set_cookie_text(cookie).await
        });
    }

    /// Sign out (for switching accounts).
    pub fn logout(&self) {
        self.run(|session| async move { session.lock().await.logout().await });
    }

    pub fn open_favorites(&self) {
        self.run(move |session| async move { session.lock().await.favorites_open().await });
    }

    pub fn import_folder(&self, media_id: i64, title: String) {
        self.run(move |session| async move {
            let folder = FavoriteFolder {
                id: media_id,
                title,
                count: 0,
            };
            session.lock().await.import_folder(folder);
        });
    }

    pub fn import_bv(&self, url: String) {
        self.run(move |session| async move { session.lock().await.import_bv(url) });
    }

    pub fn sync_pull(&self) {
        self.run(move |session| async move { session.lock().await.sync_pull(false).await });
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
        user: app.user.clone(),
        login: app.login.clone(),
        favorites: app.favorites.as_ref().map(|modal| FavoritesState {
            folders: modal.folders.clone(),
            selected: modal.selected.map(|value| value as u32),
            loading: modal.loading,
            status: modal.status.clone(),
        }),
        show_lyrics: app.show_lyrics,
        lyrics: app.lyrics.clone(),
        lyrics_status: app.lyrics_status.clone(),
        cover_key: app.cover_key.clone(),
        cache_progress: app
            .cache_progress
            .map(|(downloaded, total)| CacheProgress { downloaded, total }),
        cached_now: app.cached_now,
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
