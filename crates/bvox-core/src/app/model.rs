//! UI-agnostic state types shared between the session and the frontends.

use serde::{Deserialize, Serialize};

use crate::api::{FavoriteFolder, VideoItem};

pub struct PlayRequest {
    pub item: VideoItem,
    pub page_index: usize,
    /// Seconds to start from; non-zero when resuming a restored track.
    pub start_at: f64,
}

pub struct PlayingInfo {
    pub item: VideoItem,
    pub quality: String,
    pub page_index: usize,
    pub page_count: usize,
    pub part: String,
    pub page_duration: i64,
    pub cid: i64,
    pub pic: Option<String>,
    /// Every 分P of the loaded video, for the frontend page picker.
    pub pages: Vec<PageSummary>,
    /// Seconds the player was asked to start from (resume), zero otherwise.
    pub start_position: f64,
    /// True when playback came from the on-disk audio cache.
    pub cached: bool,
}

/// One 分P of the video currently loaded in the player.
#[derive(Clone, Debug, Serialize, Deserialize, uniffi::Record)]
pub struct PageSummary {
    pub cid: i64,
    /// Part title, falling back to `P{n}` when Bilibili omits it.
    pub title: String,
    /// Formatted `m:ss` length of this page.
    pub duration: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    None,
    Search,
    Bv,
    Favorite,
    NewPlaylist,
    RenamePlaylist,
}

/// Which list the main panel shows: the favorite folders (playlists) or the
/// current song queue (playlist contents / search results).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainView {
    Library,
    Queue,
}

pub struct PlaylistPicker {
    /// Track waiting to be added to a folder, identified by BV id so the
    /// selection cannot go stale while folders load.
    pub target_bvid: String,
    pub selected: Option<usize>,
}
pub struct BvImport {
    pub item: VideoItem,
    pub message: String,
    pub error: bool,
    /// Folder the video was added to, when a playlist was open.
    pub playlist: Option<FavoriteFolder>,
    /// Whether the account can create a playlist for a parked import.
    pub logged_in: bool,
}

/// A BV import that has no playlist to land in yet. Both frontends prompt for
/// a new playlist and then move this track into it.
#[derive(Clone, Debug)]
pub struct PendingImport {
    pub bvid: String,
    pub title: String,
}

#[derive(Clone, uniffi::Record)]
pub struct LoginModal {
    pub qr: Vec<Vec<bool>>,
    pub status: String,
}
