//! Public data types returned by the Bilibili API.

use serde::{Deserialize, Serialize};

use crate::cache::CachedAudio;

#[derive(Clone, Debug, Serialize, Deserialize, uniffi::Record)]
pub struct VideoItem {
    /// Bilibili `aid`; required to add/remove the video from a favorite folder.
    pub aid: i64,
    pub bvid: String,
    pub title: String,
    pub author: String,
    pub duration: String,
    pub pic: Option<String>,
}

/// One page of search hits plus the pagination metadata Bilibili reports.
#[derive(Clone, Debug)]
pub struct SearchPage {
    pub items: Vec<VideoItem>,
    /// Page number this result belongs to (1-based).
    pub page: u32,
    /// Total number of pages the query has.
    pub pages: u32,
    /// Total number of hits the query has.
    pub total: u64,
}

#[derive(Clone, Debug)]
pub struct AudioStream {
    pub url: String,
    pub id: i64,
    pub bandwidth: i64,
}

/// Where the audio for one page comes from: a finished cache file or the
/// (expiring) DASH URL returned by `playurl`.
#[derive(Clone, Debug)]
pub enum AudioSource {
    Cached(CachedAudio),
    Remote(AudioStream),
}

impl AudioSource {
    pub fn quality_id(&self) -> i64 {
        match self {
            Self::Cached(cached) => cached.quality_id,
            Self::Remote(stream) => stream.id,
        }
    }

    pub fn bandwidth(&self) -> i64 {
        match self {
            Self::Cached(cached) => cached.bandwidth,
            Self::Remote(stream) => stream.bandwidth,
        }
    }

    pub fn is_cached(&self) -> bool {
        matches!(self, Self::Cached(_))
    }

    /// A local file path or an HTTP URL, ready to hand to mpv.
    pub fn target(&self) -> String {
        match self {
            Self::Cached(cached) => cached.path.to_string_lossy().into_owned(),
            Self::Remote(stream) => stream.url.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, uniffi::Record)]
pub struct FavoriteFolder {
    pub id: i64,
    pub title: String,
    pub count: i64,
}

/// A favorite folder plus its playable tracks, loaded in one API pass.
#[derive(Clone, Debug)]
pub struct FavoriteContent {
    pub folder: FavoriteFolder,
    pub items: Vec<VideoItem>,
}

#[derive(Clone, Debug)]
pub struct UserInfo {
    pub name: String,
    /// Avatar URL from the nav response (http on some accounts).
    pub face: Option<String>,
    /// True for an active 大会员 (VIP) account, which unlocks Dolby/Hi-Res
    /// audio tracks.
    pub vip: bool,
}
#[derive(Clone, Debug)]
pub struct QrLogin {
    pub url: String,
    pub key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QrPhase {
    Waiting,
    Scanned,
    Expired,
    Success,
}
