//! Actions produced by key handling; the runner maps them to session calls.

//! TUI-side state: the keymap and render-only bits (table selection state,
//! decoded cover image). Session state lives in `bvox_core::app::App`.

use bvox_core::api::{AudioQuality, FavoriteFolder, PlayMode};
use bvox_core::app::PlayRequest;
use bvox_core::cache::CacheMode;
pub enum Action {
    None,
    Quit,
    Search(String),
    LoadMoreSearch,
    ImportBv(String),
    ImportPublicPlaylist(String),
    Play(PlayRequest),
    Pause,
    Seek(f64),
    Volume(f64),
    SetQuality(AudioQuality),
    SetPlayMode(PlayMode),
    Login(u64),
    OpenLibrary,
    OpenQueue,
    OpenPlaylist(FavoriteFolder),
    CreatePlaylist(String),
    CancelPendingImport,
    RenamePlaylist { folder_id: i64, title: String },
    DeletePlaylist(FavoriteFolder),
    SetPlaylistHidden { folder_id: i64, hidden: bool },
    ToggleShowHidden,
    RefreshLyrics,
    SetCacheMode(CacheMode),
    ClearCache,
    BeginAddTrack(String),
    AddToPlaylist { folder_id: i64, bvid: String },
    RemoveFromPlaylist { folder_id: i64, bvid: String },
}

/// Destructive action waiting for `y`/Enter confirmation.
pub enum ConfirmAction {
    DeletePlaylist(FavoriteFolder),
    RemoveTrack {
        folder_id: i64,
        bvid: String,
        title: String,
    },
    ClearCache,
}

impl ConfirmAction {
    pub fn message(&self) -> String {
        match self {
            Self::DeletePlaylist(folder) => {
                format!("删除歌单「{}」？视频不会被删除。", folder.title)
            }
            Self::RemoveTrack { title, .. } => {
                format!("从当前歌单移除「{title}」？")
            }
            Self::ClearCache => "清理全部缓存？正在播放的缓存文件会保留。".to_string(),
        }
    }
}
