//! Read a favorite folder's metadata and contents by link or media id.
//!
//! The same resource-list endpoint serves the account's own folders and other
//! users' public folders; the difference is only how the caller obtained the
//! id and that public folders must stay read-only in the app.

use anyhow::{Context, Result, bail};

use super::favorite_link::{is_short_link, parse_favorite_media_id};
use super::playlists::FavoriteItemsResponse;
use super::{ApiClient, FavoriteContent, FavoriteFolder};

impl ApiClient {
    /// Turn pasted input into a folder id. Full links are parsed locally;
    /// `b23.tv` / `bili2233.cn` short links need one redirect round trip.
    pub async fn resolve_favorite_media_id(&self, input: &str) -> Result<i64> {
        if let Some(id) = parse_favorite_media_id(input) {
            return Ok(id);
        }

        let input = input.trim();
        if is_short_link(input) {
            let expanded = self.expand_short_link(input).await?;
            if let Some(id) = parse_favorite_media_id(&expanded) {
                return Ok(id);
            }
        }
        bail!("无法识别公开收藏夹，请粘贴完整收藏夹链接或 media_id")
    }

    /// Load a folder's metadata and every playable video. Works for public
    /// folders of other users; private folders surface the API's error.
    pub async fn favorite_contents(&self, media_id: i64) -> Result<FavoriteContent> {
        let mut items = Vec::new();
        let mut folder: Option<FavoriteFolder> = None;

        for page in 1..=100 {
            let url = format!(
                "https://api.bilibili.com/x/v3/fav/resource/list?media_id={media_id}&pn={page}&ps=20&platform=web"
            );
            let resp: FavoriteItemsResponse = self
                .get_json(&url, None)
                .await
                .context("获取收藏夹内容失败")?;

            if resp.code != 0 {
                let message = resp.message.unwrap_or_else(|| "未知错误".to_string());
                if resp.code == -403 {
                    bail!("无法读取该收藏夹（可能是私有或不存在）: {message}");
                }
                bail!("获取收藏夹内容失败 {}: {message}", resp.code);
            }

            let Some(data) = resp.data else {
                break;
            };
            if folder.is_none() {
                folder = data.info.and_then(|info| {
                    let title = info.title.unwrap_or_default();
                    let title = title.trim();
                    (!title.is_empty()).then(|| FavoriteFolder {
                        id: media_id,
                        title: title.to_string(),
                        count: info.media_count.unwrap_or(0),
                    })
                });
            }

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
            let _ = crate::playlist_store::save_items(&self.paths.data_dir, mid, media_id, &items);
        }

        let folder = folder.unwrap_or_else(|| FavoriteFolder {
            id: media_id,
            title: format!("收藏夹 {media_id}"),
            count: items.len() as i64,
        });
        Ok(FavoriteContent { folder, items })
    }
}
