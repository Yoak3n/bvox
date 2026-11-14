//! Favorite folders (playlists): listing, contents and management.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;

use super::format_secs;
use super::sign::encode_component;
use super::text::{clean_title, normalize_url};
use super::transport::check_response;
use super::video::Owner;
use super::{ApiClient, FavoriteFolder, VideoItem};

impl ApiClient {
    pub async fn favorite_folders(&self) -> Result<Vec<FavoriteFolder>> {
        let mid = self.mid().await.context("未登录，请先登录")?;
        let url = format!("https://api.bilibili.com/x/v3/fav/folder/created/list-all?up_mid={mid}");
        let resp: FavoriteFoldersResponse =
            self.get_json(&url, None).await.context("获取收藏夹失败")?;

        if resp.code != 0 {
            bail!(
                "获取收藏夹失败 {}: {}",
                resp.code,
                resp.message.unwrap_or_else(|| "未知错误".to_string())
            );
        }

        let folders = playlist_folders(resp.data.and_then(|data| data.list).unwrap_or_default());
        let _ = crate::playlists::save_folders(&self.paths.data_dir, mid, &folders);
        Ok(folders)
    }

    /// Last successfully fetched folder list for the current account.
    pub async fn cached_favorite_folders(&self) -> Option<Vec<FavoriteFolder>> {
        let mid = self.mid().await?;
        crate::playlists::load_folders(&self.paths.data_dir, mid)
    }

    /// Last successfully fetched contents of one folder.
    pub async fn cached_favorite_items(&self, media_id: i64) -> Option<Vec<VideoItem>> {
        let mid = self.mid().await?;
        crate::playlists::load_items(&self.paths.data_dir, mid, media_id)
    }

    /// Persist the folder list after a local mutation (create/rename/delete).
    pub async fn save_playlist_cache(&self, folders: &[FavoriteFolder]) {
        if let Some(mid) = self.mid().await {
            let _ = crate::playlists::save_folders(&self.paths.data_dir, mid, folders);
        }
    }

    /// Import every playable video from a favorite folder (up to 100 pages).
    pub async fn favorite_items(&self, media_id: i64) -> Result<Vec<VideoItem>> {
        let mut items = Vec::new();

        for page in 1..=100 {
            let url = format!(
                "https://api.bilibili.com/x/v3/fav/resource/list?media_id={media_id}&pn={page}&ps=20&platform=web"
            );
            let resp: FavoriteItemsResponse = self
                .get_json(&url, None)
                .await
                .context("获取收藏夹内容失败")?;

            if resp.code != 0 {
                bail!(
                    "获取收藏夹内容失败 {}: {}",
                    resp.code,
                    resp.message.unwrap_or_else(|| "未知错误".to_string())
                );
            }

            let Some(data) = resp.data else {
                break;
            };
            let medias = data.medias.unwrap_or_default();
            if medias.is_empty() {
                break;
            }

            for media in medias {
                if let Some(item) = media.into_item() {
                    items.push(item);
                }
            }

            if !data.has_more {
                break;
            }
        }

        if let Some(mid) = self.mid().await {
            let _ = crate::playlists::save_items(&self.paths.data_dir, mid, media_id, &items);
        }
        Ok(items)
    }

    async fn create_favorite_folder(&self, title: &str, intro: &str, private: bool) -> Result<i64> {
        let csrf = self.csrf_token().await?;
        let body = folder_create_body(title, intro, private, &csrf);
        let response = self
            .post_form("https://api.bilibili.com/x/v3/fav/folder/add", body)
            .await
            .context("创建收藏夹失败")?;

        check_response(&response, "创建收藏夹失败")?;

        if let Some(id) = response
            .get("data")
            .and_then(|data| data.get("id"))
            .and_then(Value::as_i64)
        {
            return Ok(id);
        }

        self.favorite_folders()
            .await?
            .into_iter()
            .find(|folder| folder.title == clean_title(title))
            .map(|folder| folder.id)
            .context("收藏夹已创建但无法获取 id")
    }

    /// Create a private playlist. Everything this app creates stays out of the
    /// public profile by default; privacy can still be changed on the website.
    pub async fn create_playlist(&self, title: &str) -> Result<FavoriteFolder> {
        let title = title.trim();
        if title.is_empty() {
            bail!("歌单名不能为空");
        }
        let id = self
            .create_favorite_folder(title, "由 Bvox 创建", true)
            .await?;
        Ok(FavoriteFolder {
            id,
            title: title.to_string(),
            count: 0,
        })
    }

    /// Rename a favorite folder. Only the title is sent so the folder's
    /// privacy/intro configured on the website stay untouched.
    pub async fn rename_playlist(&self, media_id: i64, title: &str) -> Result<()> {
        let title = title.trim();
        if title.is_empty() {
            bail!("歌单名不能为空");
        }
        let csrf = self.csrf_token().await?;
        let body = folder_edit_body(media_id, title, &csrf);
        let response = self
            .post_form("https://api.bilibili.com/x/v3/fav/folder/edit", body)
            .await
            .context("重命名歌单失败")?;
        check_response(&response, "重命名歌单失败")
    }

    /// Delete a favorite folder. The videos themselves are not deleted.
    pub async fn delete_playlist(&self, media_id: i64) -> Result<()> {
        let csrf = self.csrf_token().await?;
        let body = folder_delete_body(media_id, &csrf);
        let response = self
            .post_form("https://api.bilibili.com/x/v3/fav/folder/del", body)
            .await
            .context("删除歌单失败")?;
        check_response(&response, "删除歌单失败")
    }

    /// Add or remove one video from a folder. Returns `false` when the folder
    /// already was (or already wasn't) in the requested state, so callers can
    /// keep their cached song counts accurate.
    pub async fn set_favorite(&self, folder_id: i64, aid: i64, add: bool) -> Result<bool> {
        let csrf = self.csrf_token().await?;
        let body = deal_body(folder_id, aid, add, &csrf);
        let action = if add {
            "加入歌单失败"
        } else {
            "从歌单移除失败"
        };
        let response = self
            .post_form("https://api.bilibili.com/x/v3/fav/resource/deal", body)
            .await
            .context(action)?;

        let code = response.get("code").and_then(Value::as_i64).unwrap_or(-1);
        if code == 0 {
            return Ok(true);
        }
        // 11201: already favorited; 11202: not favorited (remove no-op).
        if (add && code == 11201) || (!add && code == 11202) {
            return Ok(false);
        }
        bail!(
            "{action} {code}: {}",
            response
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("未知错误")
        )
    }
}
pub(super) fn folder_create_body(title: &str, intro: &str, private: bool, csrf: &str) -> String {
    format!(
        "title={}&intro={}&privacy={}&csrf={}",
        encode_component(title),
        encode_component(intro),
        if private { "1" } else { "0" },
        encode_component(csrf)
    )
}

pub(super) fn folder_edit_body(media_id: i64, title: &str, csrf: &str) -> String {
    format!(
        "media_id={media_id}&title={}&csrf={}",
        encode_component(title),
        encode_component(csrf)
    )
}

pub(super) fn folder_delete_body(media_id: i64, csrf: &str) -> String {
    format!("media_ids={media_id}&csrf={}", encode_component(csrf))
}

pub(super) fn deal_body(folder_id: i64, aid: i64, add: bool, csrf: &str) -> String {
    let (add_ids, del_ids) = if add {
        (folder_id.to_string(), String::new())
    } else {
        (String::new(), folder_id.to_string())
    };
    format!(
        "rid={aid}&type=2&add_media_ids={add_ids}&del_media_ids={del_ids}&csrf={}",
        encode_component(csrf)
    )
}
/// Normalize the folder list returned by Bilibili (titles may contain HTML).
pub(super) fn playlist_folders(list: Vec<FavoriteFolderRaw>) -> Vec<FavoriteFolder> {
    list.into_iter()
        .map(|folder| FavoriteFolder {
            id: folder.id,
            title: clean_title(&folder.title),
            count: folder.media_count,
        })
        .collect()
}
#[derive(Deserialize)]
pub(super) struct FavoriteFoldersResponse {
    #[serde(default)]
    pub(super) code: i64,
    pub(super) message: Option<String>,
    pub(super) data: Option<FavoriteFoldersData>,
}

#[derive(Deserialize)]
pub(super) struct FavoriteFoldersData {
    pub(super) list: Option<Vec<FavoriteFolderRaw>>,
}

#[derive(Deserialize)]
pub(super) struct FavoriteFolderRaw {
    pub(super) id: i64,
    #[serde(default)]
    pub(super) title: String,
    #[serde(default)]
    pub(super) media_count: i64,
}

#[derive(Deserialize)]
pub(super) struct FavoriteItemsResponse {
    #[serde(default)]
    pub(super) code: i64,
    pub(super) message: Option<String>,
    pub(super) data: Option<FavoriteItemsData>,
}

#[derive(Deserialize)]
pub(super) struct FavoriteItemsData {
    pub(super) medias: Option<Vec<FavoriteMediaRaw>>,
    #[serde(default)]
    pub(super) has_more: bool,
}

#[derive(Deserialize)]
pub(super) struct FavoriteMediaRaw {
    /// Bilibili `aid` of the video inside the folder.
    #[serde(default)]
    pub(super) id: i64,
    #[serde(default)]
    pub(super) attr: i64,
    pub(super) bvid: Option<String>,
    #[serde(default)]
    pub(super) title: String,
    #[serde(default)]
    pub(super) duration: i64,
    pub(super) cover: Option<String>,
    pub(super) upper: Option<Owner>,
    #[serde(default, rename = "type")]
    pub(super) media_type: i64,
}

impl FavoriteMediaRaw {
    pub(super) fn into_item(self) -> Option<VideoItem> {
        // `attr != 0` marks invalid/deleted videos; only regular video
        // entries (`type == 2`) are playable.
        if self.media_type != 2 || self.attr != 0 {
            return None;
        }
        let bvid = self.bvid.filter(|bvid| !bvid.is_empty())?;
        Some(VideoItem {
            aid: self.id,
            bvid,
            title: clean_title(&self.title),
            author: self.upper.map(|upper| upper.name).unwrap_or_default(),
            duration: format_secs(self.duration),
            pic: self.cover.as_deref().map(normalize_url),
        })
    }
}
