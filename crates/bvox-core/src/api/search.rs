//! WBI-signed music search.

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::sign::{mixin_key, signed_query};
use super::text::{clean_title, normalize_url};
use super::video::normalize_duration;
use super::{ApiClient, SearchPage, VideoItem};

/// Bilibili's music top-level partition ("音乐", zone id 3). Searches are
/// restricted to it so results stay playable music instead of videos from
/// every zone.
const MUSIC_TID: &str = "3";
/// Default hit count per search page when the response omits `pagesize`.
const SEARCH_PAGE_SIZE: usize = 20;
impl ApiClient {
    /// Search the first page of videos. Kept for one-shot callers such as the
    /// headless CLI; interactive frontends page through [`Self::search_page`].
    pub async fn search(&self, keyword: &str) -> Result<Vec<VideoItem>> {
        Ok(self.search_page(keyword, 1).await?.items)
    }

    /// Search videos. The wbi endpoint requires a `buvid3` cookie and a signed query.
    pub async fn search_page(&self, keyword: &str, page: u32) -> Result<SearchPage> {
        let page = page.max(1);
        let (img_key, sub_key) = self.wbi_keys().await?;
        let mixin_key = mixin_key(&img_key, &sub_key);
        let buvid = self.buvid_extra().await?;

        let query = signed_query(
            vec![
                ("search_type", "video".to_string()),
                ("keyword", keyword.to_string()),
                ("page", page.to_string()),
                ("tids", MUSIC_TID.to_string()),
            ],
            &mixin_key,
        );
        let url = format!("https://api.bilibili.com/x/web-interface/wbi/search/type?{query}");

        let resp: SearchResponse = self
            .get_json(&url, buvid.as_deref())
            .await
            .context("搜索请求失败")?;

        if resp.code != 0 {
            bail!(
                "搜索失败 {}: {}",
                resp.code,
                resp.message.unwrap_or_else(|| "未知错误".to_string())
            );
        }

        let data = resp.data.unwrap_or_default();
        let items = data
            .result
            .unwrap_or_default()
            .into_iter()
            .filter_map(SearchRaw::into_item)
            .collect::<Vec<_>>();
        let page_no = if data.page == 0 { page } else { data.page };
        // Older responses occasionally omit `numPages`; a full page then
        // implies another one may exist, and an empty page ends the run.
        let pages = if data.num_pages > 0 {
            data.num_pages
        } else if !items.is_empty() {
            let size = if data.page_size > 0 {
                data.page_size as usize
            } else {
                SEARCH_PAGE_SIZE
            };
            if items.len() >= size {
                page_no + 1
            } else {
                page_no
            }
        } else {
            page_no
        };

        Ok(SearchPage {
            items,
            page: page_no,
            pages,
            total: data.num_results,
        })
    }
}
#[derive(Deserialize)]
pub(super) struct SearchResponse {
    pub(super) code: i64,
    pub(super) message: Option<String>,
    pub(super) data: Option<SearchData>,
}

#[derive(Deserialize, Default)]
pub(super) struct SearchData {
    #[serde(default)]
    pub(super) result: Option<Vec<SearchRaw>>,
    #[serde(default)]
    pub(super) page: u32,
    #[serde(default, rename = "pagesize")]
    pub(super) page_size: u32,
    #[serde(default, rename = "numPages")]
    pub(super) num_pages: u32,
    #[serde(default, rename = "numResults")]
    pub(super) num_results: u64,
}

#[derive(Deserialize)]
pub(super) struct SearchRaw {
    #[serde(default)]
    pub(super) aid: i64,
    /// Bilibili search results carry both `aid` and `id` (same value). `id` is
    /// kept separately because `alias = "id"` makes serde reject the real
    /// response with "duplicate field `aid`".
    #[serde(default)]
    pub(super) id: Option<i64>,
    pub(super) bvid: Option<String>,
    pub(super) title: Option<String>,
    pub(super) author: Option<String>,
    pub(super) duration: Option<String>,
    pub(super) pic: Option<String>,
}

impl SearchRaw {
    pub(super) fn into_item(self) -> Option<VideoItem> {
        let bvid = self.bvid?;
        if bvid.is_empty() {
            return None;
        }
        let aid = if self.aid != 0 {
            self.aid
        } else {
            self.id.unwrap_or(0)
        };
        Some(VideoItem {
            aid,
            bvid,
            title: clean_title(self.title.as_deref().unwrap_or("")),
            author: self.author.unwrap_or_default(),
            duration: normalize_duration(self.duration.as_deref().unwrap_or("")),
            pic: self.pic.as_deref().map(normalize_url),
        })
    }
}
