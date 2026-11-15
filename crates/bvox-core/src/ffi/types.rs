//! UniFFI export types: the snapshot records and error surfaced to Kotlin.

use crate::api::{AudioQuality, FavoriteFolder, LyricLine, PlayMode, VideoItem};
use crate::app::{LoginModal, PageSummary};
use crate::cache::{CacheMode, CachedTrack};

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
    /// 分P list of the loaded video; restored with the session for paused
    /// resumes, empty before the first playback of a fresh track.
    pub pages: Vec<PageSummary>,
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
    /// True when `current_playlist` belongs to another user and is read-only.
    pub current_playlist_public: bool,
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
