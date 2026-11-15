//! UI-agnostic session state.
//!
//! `App` holds every piece of state a frontend renders: the playlist library,
//! the song queue, playback progress, login, lyrics. Frontends mutate it
//! through [`Session`](crate::session::Session) commands and read it back for
//! drawing; the field set is deliberately plain data so it maps onto Compose
//! state as easily as onto a ratatui frame.

use std::collections::HashSet;

use crate::api::{AudioQuality, FavoriteFolder, LyricLine, PlayMode, VideoItem};
use crate::cache::{CacheMode, CacheStats, CachedTrack};
use shuffle::ShuffleRng;

mod cache;
mod import;
mod library;
mod login;
mod lyrics;
mod model;
mod picker;
mod playback;
mod playlist_ops;
#[cfg(test)]
mod public_playlist_tests;
mod queue;
mod search;
mod shuffle;
#[cfg(test)]
mod tests;

pub use login::qr_matrix;
pub use model::{
    BvImport, InputMode, LoginModal, MainView, PageSummary, PendingImport, PlayRequest,
    PlayingInfo, PlaylistPicker,
};

pub struct App {
    pub input: String,
    pub input_mode: InputMode,
    pub results: Vec<VideoItem>,
    pub results_title: String,
    /// Playback order: sequential, repeat-all, repeat-one or shuffle.
    pub play_mode: PlayMode,
    pub selected: Option<usize>,
    pub current: Option<usize>,
    pub status: String,
    pub error: bool,
    pub loading: bool,
    pub search_seq: u64,
    /// Keyword of the currently displayed search results, when they came from
    /// a search. Empty for playlists and BV imports.
    pub search_keyword: String,
    /// Last search page loaded and how many pages the query has (1-based).
    pub search_page: u32,
    pub search_pages: u32,
    /// Total hits reported by Bilibili for the current search.
    pub search_total: u64,
    /// True while the next search page is in flight.
    pub loading_more: bool,
    pub now_playing: Option<VideoItem>,
    pub quality: String,
    pub page: usize,
    pub page_count: usize,
    pub part: String,
    /// 分P list of the loaded video, backing the page picker. Cleared
    /// whenever the queue moves to another track; refilled on `Playing`.
    pub playing_pages: Vec<PageSummary>,
    pub current_cid: i64,
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
    pub volume: f64,
    pub has_media: bool,
    pub pending_play: bool,
    /// Consecutive load failures while skipping unavailable tracks. Reset on
    /// a successful start or when a new queue is installed.
    pub play_failures: usize,
    /// Why the last playback start failed. Exhausting the queue would
    /// otherwise overwrite the real error with a generic guess.
    pub play_error: Option<String>,
    pub playback_started: bool,
    pub eof: bool,
    /// True when a cookie is stored, even if the nav request cannot confirm
    /// the account (offline starts). Distinguishes offline from logged out.
    pub has_cookie: bool,
    pub user: Option<String>,
    /// Avatar URL for the logged-in account, when the nav response has one.
    pub user_face: Option<String>,
    /// True when the logged-in account has an active 大会员 subscription.
    pub vip: bool,
    /// Audio track preference; `Auto` follows the account entitlement.
    pub quality_pref: AudioQuality,
    /// Folder ids hidden by the user; persisted per account by `ApiClient`.
    pub hidden_playlists: HashSet<i64>,
    /// Temporarily reveal hidden folders (e.g. to unhide one).
    pub show_hidden: bool,
    pub login: Option<LoginModal>,
    pub login_seq: u64,
    pub view: MainView,
    pub playlists: Vec<FavoriteFolder>,
    pub playlists_loading: bool,
    pub playlists_seq: u64,
    pub playlist_open_seq: u64,
    pub playlist_selected: Option<usize>,
    /// The folder the current queue was opened from, if any.
    pub current_playlist: Option<FavoriteFolder>,
    /// True when the current folder belongs to another user. Public folders
    /// are read-only: tracks cannot be removed from them.
    pub current_playlist_public: bool,
    /// Add-to-playlist modal (TUI).
    pub picker: Option<PlaylistPicker>,
    /// Folder being renamed through the text input.
    pub rename_target: Option<FavoriteFolder>,
    /// Imported track waiting for the user to create a playlist for it.
    pub pending_import: Option<PendingImport>,
    /// Shuffle bookkeeping: indices already played (for previous) and the
    /// remaining bag for the current pass.
    shuffle_history: Vec<usize>,
    shuffle_remaining: Vec<usize>,
    shuffle_rng: ShuffleRng,
    pub show_lyrics: bool,
    pub lyrics: Vec<LyricLine>,
    pub lyrics_key: String,
    pub lyrics_status: String,
    pub cover_key: String,
    /// `bvid#cid#page` of the current track, used to route cache progress.
    pub playback_key: String,
    pub cache_progress: Option<(u64, Option<u64>)>,
    pub cached_now: bool,
    /// On-disk cache usage, refreshed by the session after writes/cleanup.
    pub cache_stats: Option<CacheStats>,
    /// What the cache stores (auto / small files only / off).
    pub cache_mode: CacheMode,
    /// BVIDs with completed audio files, refreshed by the session. List views
    /// use it to mark cached tracks.
    pub cached_bvids: HashSet<String>,
    /// Playable offline entries for the "cached songs" screen, newest first.
    pub cached_tracks: Vec<CachedTrack>,
}

impl App {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            input_mode: InputMode::None,
            results: Vec::new(),
            results_title: "搜索结果".to_string(),
            play_mode: PlayMode::Sequential,
            selected: None,
            current: None,
            status: "搜索 B 站音乐；登录后管理收藏夹歌单".to_string(),
            error: false,
            loading: false,
            search_seq: 0,
            search_keyword: String::new(),
            search_page: 0,
            search_pages: 0,
            search_total: 0,
            loading_more: false,
            now_playing: None,
            quality: String::new(),
            page: 0,
            page_count: 0,
            part: String::new(),
            playing_pages: Vec::new(),
            current_cid: 0,
            position: 0.0,
            duration: 0.0,
            paused: false,
            volume: 100.0,
            has_media: false,
            pending_play: false,
            play_failures: 0,
            play_error: None,
            playback_started: false,
            eof: false,
            has_cookie: false,
            user: None,
            user_face: None,
            vip: false,
            quality_pref: AudioQuality::Auto,
            hidden_playlists: HashSet::new(),
            show_hidden: false,
            login: None,
            login_seq: 0,
            view: MainView::Queue,
            playlists: Vec::new(),
            playlists_loading: false,
            playlists_seq: 0,
            playlist_open_seq: 0,
            playlist_selected: None,
            current_playlist: None,
            current_playlist_public: false,
            picker: None,
            rename_target: None,
            pending_import: None,
            shuffle_history: Vec::new(),
            shuffle_remaining: Vec::new(),
            shuffle_rng: ShuffleRng::new(),
            show_lyrics: false,
            lyrics: Vec::new(),
            lyrics_key: String::new(),
            lyrics_status: String::new(),
            cover_key: String::new(),
            playback_key: String::new(),
            cache_progress: None,
            cached_now: false,
            cache_stats: None,
            cache_mode: CacheMode::Auto,
            cached_bvids: HashSet::new(),
            cached_tracks: Vec::new(),
        }
    }

    pub fn set_status(&mut self, message: &str, error: bool) {
        self.status = message.to_string();
        self.error = error;
    }

    pub fn set_error(&mut self, error: impl std::fmt::Display) {
        self.set_status(&format!("{error:#}"), true);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
