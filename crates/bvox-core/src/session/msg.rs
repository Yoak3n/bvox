//! Message vocabulary between frontends, background tasks and the session loop.

use anyhow::Result;

use crate::api::{FavoriteContent, FavoriteFolder, SearchPage, VideoItem};
use crate::app::{BvImport, PendingImport, PlayingInfo};

/// Internal message vocabulary. `Key`/`Quit`/`Wake` stayed frontend-side;
/// everything else is session business.
pub enum Msg {
    /// Nudge the frontend to re-read state (no other effect).
    StateChanged,
    SearchDone {
        seq: u64,
        keyword: String,
        result: Result<SearchPage, String>,
    },
    SearchMoreDone {
        seq: u64,
        result: Result<SearchPage, String>,
    },
    Playing(PlayingInfo),
    PlayFailed(String),
    PlayerState(crate::player::PlayerState),
    LoginQr {
        seq: u64,
        matrix: Vec<Vec<bool>>,
    },
    LoginPhase {
        seq: u64,
        status: String,
    },
    LoginSuccess {
        seq: u64,
        user: Option<crate::api::UserInfo>,
    },
    LoginFailed {
        seq: u64,
        message: String,
    },
    PlaylistsLoaded {
        seq: u64,
        result: Result<(Vec<FavoriteFolder>, Vec<i64>), String>,
    },
    PlaylistOpened {
        seq: u64,
        folder: FavoriteFolder,
        result: Result<Vec<VideoItem>, String>,
        /// True when the contents came from the offline cache.
        offline: bool,
    },
    /// Another user's public folder, loaded as a read-only queue.
    PublicPlaylistOpened {
        seq: u64,
        result: Result<FavoriteContent, String>,
    },
    PlaylistHidden {
        folder_id: i64,
        hidden: bool,
        result: Result<(), String>,
    },
    PlaylistCreated {
        result: Result<FavoriteFolder, String>,
        /// Track waiting for this playlist; `None` for a manual create.
        pending: Option<PendingImport>,
    },
    PlaylistRenamed {
        folder_id: i64,
        title: String,
        result: Result<(), String>,
    },
    PlaylistDeleted {
        folder_id: i64,
        result: Result<(), String>,
    },
    TrackAdded {
        folder_id: i64,
        changed: bool,
        result: Result<(), String>,
    },
    TrackRemoved {
        folder_id: i64,
        bvid: String,
        changed: bool,
        result: Result<(), String>,
    },
    CoverReady {
        key: String,
        result: Result<Vec<u8>, String>,
    },
    CacheProgress {
        key: String,
        downloaded: u64,
        total: Option<u64>,
    },
    CacheDone {
        key: String,
        ok: bool,
    },
    LyricsReady {
        seq: u64,
        key: String,
        result: Result<Vec<crate::api::LyricLine>, String>,
    },
    UserInfo(Option<crate::api::UserInfo>),
    BvImported(Result<BvImport, String>),
}

/// Events a frontend observes from [`Session::pump`].
pub enum CoreEvent {
    /// State changed; re-read the app snapshot / redraw.
    StateChanged,
    /// Cover bytes arrived; `key` is `bvid#cid#page` of the track it belongs to.
    CoverReady { key: String, bytes: Vec<u8> },
    /// The session channel closed; the frontend should exit.
    Closed,
}
