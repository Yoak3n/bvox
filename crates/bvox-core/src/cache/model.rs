//! Cache data model: modes, record types and formatting.

use std::time::Duration;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::{LyricLine, VideoDetail};

pub(super) const VIEW_TTL: Duration = Duration::from_secs(24 * 60 * 60);
pub(super) const STALE_PART_AGE: Duration = Duration::from_secs(24 * 60 * 60);
/// Bump when lyric fetching/parsing changes so old files are discarded
/// instead of served forever. Version 2 drops lyrics poisoned by the
/// unsigned `/x/player/v2` endpoint returning another video's subtitle.
pub(super) const LYRICS_VERSION: u32 = 2;

/// What the on-disk cache is allowed to store.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, uniffi::Enum)]
#[repr(u8)]
pub enum CacheMode {
    /// 自动缓存：音频、封面、歌词和视频信息都落盘。
    #[default]
    Auto = 0,
    /// 仅音频：只缓存音频和播放所需的曲目信息，不保存封面和歌词。
    AudioOnly = 1,
    /// 仅小文件：音频始终在线播放，只缓存封面、歌词和视频信息。
    SmallFiles = 2,
    /// 关闭：不读也不写缓存，已有文件保留到手动清理。
    Off = 3,
}

impl CacheMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "自动缓存",
            Self::AudioOnly => "仅音频",
            Self::SmallFiles => "仅小文件",
            Self::Off => "关闭",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Auto => "音频、封面、歌词和视频信息都缓存，下次播放直接用本地文件",
            Self::AudioOnly => "只缓存音频和播放所需的曲目信息，不保存封面和歌词",
            Self::SmallFiles => "音频始终在线播放，只缓存封面、歌词和视频信息",
            Self::Off => "不读也不写缓存，已有文件保留到手动清理",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::AudioOnly,
            Self::AudioOnly => Self::SmallFiles,
            Self::SmallFiles => Self::Off,
            Self::Off => Self::Auto,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "audio_only" | "audio-only" | "audio" => Some(Self::AudioOnly),
            "small_files" | "small-files" | "small" => Some(Self::SmallFiles),
            "off" => Some(Self::Off),
            _ => None,
        }
    }

    pub(super) fn as_u8(self) -> u8 {
        self as u8
    }

    pub(super) fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::AudioOnly,
            2 => Self::SmallFiles,
            3 => Self::Off,
            _ => Self::Auto,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CachedAudio {
    pub path: PathBuf,
    pub quality_id: i64,
    pub bandwidth: i64,
}

/// One playable entry for the offline "cached songs" list. Built by joining a
/// completed audio file with its cached `view` metadata; files without view
/// metadata are skipped because playback still needs the page/cid mapping.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct CachedTrack {
    pub bvid: String,
    pub cid: i64,
    /// Page index inside a multi-P video (0 for single-P).
    pub page_index: u32,
    pub aid: i64,
    pub title: String,
    pub author: String,
    pub duration: String,
    pub pic: Option<String>,
    pub quality: String,
    /// Empty for single-P videos.
    pub part: String,
}

#[derive(Serialize, Deserialize)]
pub(super) struct CachedView {
    pub(super) fetched_at: u64,
    pub(super) detail: VideoDetail,
}

#[derive(Serialize, Deserialize)]
pub(super) struct CachedLyrics {
    pub(super) version: u32,
    pub(super) lines: Vec<LyricLine>,
}

/// On-disk cache usage, grouped by what the files are used for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub total_bytes: u64,
    /// Configured cap in bytes; 0 when the cache is disabled.
    pub limit_bytes: u64,
    pub audio_bytes: u64,
    pub cover_bytes: u64,
    pub lyrics_bytes: u64,
    pub view_bytes: u64,
    pub file_count: u64,
}
