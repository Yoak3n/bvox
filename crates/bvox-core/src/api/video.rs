//! Video detail and share-link resolution.

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::format_secs;
use super::sign::encode_component;
use super::text::normalize_url;
use super::{ApiClient, REFERER, UA, VideoDetail, VideoPage};

impl ApiClient {
    /// Follow a share short link (`b23.tv`, `bili2233.cn`) to the page it
    /// points at. reqwest follows the redirect chain, so one request is
    /// enough; the final URL is what callers parse for a BV id.
    pub async fn resolve_share_link(&self, link: &str) -> Result<String> {
        let response = self
            .http
            .get(link)
            .header("User-Agent", UA)
            .header("Referer", REFERER)
            .send()
            .await
            .context("打开分享链接失败")?;
        Ok(response.url().to_string())
    }

    /// Resolve a BV id into its first page cid and metadata.
    ///
    /// Fresh cache entries avoid the request entirely. If the request fails
    /// (offline, risk control) a stale entry is still usable, which keeps
    /// already-cached songs playable.
    pub async fn view(&self, bvid: &str) -> Result<VideoDetail> {
        if let Some(detail) = self.cache.get_view(bvid) {
            return Ok(detail);
        }

        match self.fetch_view(bvid).await {
            Ok(detail) => {
                self.cache.put_view(bvid, &detail);
                Ok(detail)
            }
            Err(error) => match self.cache.get_stale_view(bvid) {
                Some(detail) => Ok(detail),
                None => Err(error),
            },
        }
    }

    async fn fetch_view(&self, bvid: &str) -> Result<VideoDetail> {
        let url = format!(
            "https://api.bilibili.com/x/web-interface/view?bvid={}",
            encode_component(bvid)
        );
        let resp: ViewResponse = self
            .get_json(&url, None)
            .await
            .context("获取视频信息失败")?;

        if resp.code != 0 {
            bail!(
                "获取视频信息失败 {}: {}",
                resp.code,
                resp.message.unwrap_or_else(|| "未知错误".to_string())
            );
        }

        let data = resp.data.context("视频信息为空")?;
        let pages = data
            .pages
            .into_iter()
            .map(|page| VideoPage {
                cid: page.cid,
                duration: page.duration,
                part: page.part,
            })
            .collect::<Vec<_>>();

        // For multi-page videos the top-level cid/duration describe the first
        // page, but duration is the sum of all pages. Prefer the page values.
        let cid = pages.first().map(|page| page.cid).unwrap_or(data.cid);
        let duration = pages
            .first()
            .map(|page| page.duration)
            .filter(|duration| *duration > 0)
            .unwrap_or(data.duration);

        Ok(VideoDetail {
            bvid: data.bvid,
            aid: data.aid,
            cid,
            title: data.title,
            author: data.owner.name,
            duration,
            pages,
            pic: data.pic.as_deref().map(normalize_url),
        })
    }
}
/// Search results use unpadded durations like `419:2`; normalize them to `6:59:02`.
pub(super) fn normalize_duration(raw: &str) -> String {
    let parts = raw.split(':').collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 3 {
        return raw.to_string();
    }

    let mut total = 0i64;
    for part in parts {
        let Ok(value) = part.parse::<i64>() else {
            return raw.to_string();
        };
        total = total * 60 + value;
    }
    format_secs(total)
}
#[derive(Deserialize)]
pub(super) struct ViewResponse {
    pub(super) code: i64,
    pub(super) message: Option<String>,
    pub(super) data: Option<ViewData>,
}

#[derive(Deserialize)]
pub(super) struct ViewData {
    pub(super) bvid: String,
    #[serde(default)]
    pub(super) aid: i64,
    pub(super) cid: i64,
    pub(super) title: String,
    pub(super) duration: i64,
    pub(super) owner: Owner,
    pub(super) pic: Option<String>,
    #[serde(default)]
    pub(super) pages: Vec<ViewPage>,
}

#[derive(Deserialize)]
pub(super) struct ViewPage {
    pub(super) cid: i64,
    #[serde(default)]
    pub(super) duration: i64,
    #[serde(default)]
    pub(super) part: String,
}

#[derive(Deserialize)]
pub(super) struct Owner {
    #[serde(default)]
    pub(super) name: String,
}
