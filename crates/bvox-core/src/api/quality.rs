//! Audio quality and playback order preferences.

/// Preferred audio track for playback.
///
/// `Auto` follows the account entitlement: premium accounts get Hi-Res/Dolby
/// when the video has them, everyone else stays on AAC. The explicit
/// variants override that, and any premium choice falls back to AAC when the
/// account has no access or the stream is rejected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, uniffi::Enum)]
pub enum AudioQuality {
    #[default]
    Auto,
    Aac,
    Dolby,
    HiRes,
}

impl AudioQuality {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "自动",
            Self::Aac => "AAC",
            Self::Dolby => "Dolby",
            Self::HiRes => "Hi-Res",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Aac,
            Self::Aac => Self::Dolby,
            Self::Dolby => Self::HiRes,
            Self::HiRes => Self::Auto,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Aac => "aac",
            Self::Dolby => "dolby",
            Self::HiRes => "hires",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "aac" => Some(Self::Aac),
            "dolby" => Some(Self::Dolby),
            "hires" => Some(Self::HiRes),
            _ => None,
        }
    }
}

/// Premium tracks (Dolby / Hi-Res) require an active 大会员 entitlement.
pub(super) fn is_premium_audio(id: i64) -> bool {
    matches!(id, 30250 | 30251)
}

/// How playback continues after the current track ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, uniffi::Enum)]
pub enum PlayMode {
    /// 顺序播放：播完列表就停。
    #[default]
    Sequential,
    /// 列表循环：播完回到第一首。
    RepeatAll,
    /// 单曲循环：反复播放当前曲目。
    RepeatOne,
    /// 随机播放：按随机顺序播放，上一首可回退。
    Shuffle,
}

impl PlayMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sequential => "顺序播放",
            Self::RepeatAll => "列表循环",
            Self::RepeatOne => "单曲循环",
            Self::Shuffle => "随机播放",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Sequential => Self::RepeatAll,
            Self::RepeatAll => Self::RepeatOne,
            Self::RepeatOne => Self::Shuffle,
            Self::Shuffle => Self::Sequential,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Sequential => "sequential",
            Self::RepeatAll => "repeat_all",
            Self::RepeatOne => "repeat_one",
            Self::Shuffle => "shuffle",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "sequential" => Some(Self::Sequential),
            "repeat_all" => Some(Self::RepeatAll),
            "repeat_one" => Some(Self::RepeatOne),
            "shuffle" => Some(Self::Shuffle),
            _ => None,
        }
    }
}
pub fn quality_label(id: i64) -> &'static str {
    match id {
        30216 => "64K",
        30232 => "132K",
        30280 => "192K",
        30250 => "Dolby",
        30251 => "Hi-Res",
        _ => "AAC",
    }
}
