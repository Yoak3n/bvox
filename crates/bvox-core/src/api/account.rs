//! Account info and per-account preferences (audio quality, play mode,
//! hidden playlists).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use super::transport::WbiImg;
use super::{ApiClient, AudioQuality, PlayMode, UserInfo};

const AUDIO_QUALITY_FILE_NAME: &str = "audio_quality";
const PLAY_MODE_FILE_NAME: &str = "play_mode";
const HIDDEN_PLAYLISTS_FILE_NAME: &str = "hidden_playlists.json";
impl ApiClient {
    /// Audio quality preference persisted next to the cookie.
    pub fn audio_quality(&self) -> AudioQuality {
        std::fs::read_to_string(self.paths.data_dir.join(AUDIO_QUALITY_FILE_NAME))
            .ok()
            .and_then(|value| AudioQuality::parse(value.trim()))
            .unwrap_or_default()
    }

    pub fn save_audio_quality(&self, quality: AudioQuality) {
        let path = self.paths.data_dir.join(AUDIO_QUALITY_FILE_NAME);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, quality.as_str());
    }

    /// Playback order preference, persisted next to the cookie.
    pub fn play_mode(&self) -> PlayMode {
        std::fs::read_to_string(self.paths.data_dir.join(PLAY_MODE_FILE_NAME))
            .ok()
            .and_then(|value| PlayMode::parse(value.trim()))
            .unwrap_or_default()
    }

    pub fn save_play_mode(&self, mode: PlayMode) {
        let path = self.paths.data_dir.join(PLAY_MODE_FILE_NAME);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, mode.as_str());
    }

    /// Favorite folder ids hidden by the current account. Stored next to the
    /// cookie, keyed by `DedeUserID`, so switching accounts keeps each set.
    pub async fn hidden_playlists(&self) -> Vec<i64> {
        let Some(mid) = self.mid().await else {
            return Vec::new();
        };
        let map = load_hidden_playlists(&self.paths.data_dir);
        hidden_ids_for(&map, mid)
    }

    /// Hide or unhide one favorite folder for the current account and persist
    /// the list locally.
    pub async fn set_playlist_hidden(&self, folder_id: i64, hidden: bool) -> Result<()> {
        let mid = self.mid().await.context("未登录，无法保存隐藏歌单")?;
        let _guard = self.hidden_playlists_lock.lock().await;
        let mut map = load_hidden_playlists(&self.paths.data_dir);
        set_hidden_id(&mut map, mid, folder_id, hidden);
        save_hidden_playlists(&self.paths.data_dir, &map)
    }

    pub async fn user_info(&self) -> Result<Option<UserInfo>> {
        let resp: NavResponse = self
            .get_json("https://api.bilibili.com/x/web-interface/nav", None)
            .await?;
        if resp.code != 0 {
            return Ok(None);
        }
        let Some(data) = resp.data else {
            return Ok(None);
        };
        if !data.is_login {
            return Ok(None);
        }
        Ok(Some(UserInfo {
            name: data.uname.unwrap_or_default(),
            face: data
                .face
                .map(|face| face.trim().to_string())
                .filter(|face| !face.is_empty()),
            vip: data.vip_status > 0 || data.vip.is_some_and(|vip| vip.status > 0),
        }))
    }
}
/// Hidden folder ids keyed by the account's `DedeUserID`.
pub(super) type HiddenPlaylists = BTreeMap<String, Vec<i64>>;

pub(super) fn hidden_playlists_path(data_dir: &Path) -> PathBuf {
    data_dir.join(HIDDEN_PLAYLISTS_FILE_NAME)
}

pub(super) fn load_hidden_playlists(data_dir: &Path) -> HiddenPlaylists {
    std::fs::read_to_string(hidden_playlists_path(data_dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub(super) fn save_hidden_playlists(data_dir: &Path, map: &HiddenPlaylists) -> Result<()> {
    std::fs::create_dir_all(data_dir).context("创建配置目录失败")?;
    let bytes = serde_json::to_vec_pretty(map).context("序列化隐藏歌单失败")?;
    let path = hidden_playlists_path(data_dir);
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, bytes).context("写入隐藏歌单失败")?;
    std::fs::rename(&tmp, &path).context("提交隐藏歌单失败")
}

pub(super) fn hidden_ids_for(map: &HiddenPlaylists, mid: i64) -> Vec<i64> {
    map.get(&mid.to_string()).cloned().unwrap_or_default()
}

pub(super) fn set_hidden_id(map: &mut HiddenPlaylists, mid: i64, folder_id: i64, hidden: bool) {
    let ids = map.entry(mid.to_string()).or_default();
    if hidden {
        if !ids.contains(&folder_id) {
            ids.push(folder_id);
        }
    } else {
        ids.retain(|id| *id != folder_id);
    }
}
#[derive(Deserialize)]
pub(super) struct NavResponse {
    #[serde(default)]
    pub(super) code: i64,
    pub(super) data: Option<NavData>,
}

#[derive(Deserialize)]
pub(super) struct NavData {
    // The API returns camelCase here (`isLogin`) while the rest of the
    // payload uses snake_case.
    #[serde(rename = "isLogin", default)]
    pub(super) is_login: bool,
    pub(super) uname: Option<String>,
    pub(super) face: Option<String>,
    /// 1 for an active 大会员; the nested `vip.status` carries the same flag.
    #[serde(rename = "vipStatus", default)]
    pub(super) vip_status: i64,
    #[serde(default)]
    pub(super) vip: Option<VipInfo>,
    pub(super) wbi_img: Option<WbiImg>,
}

#[derive(Deserialize)]
pub(super) struct VipInfo {
    #[serde(default)]
    pub(super) status: i64,
}
