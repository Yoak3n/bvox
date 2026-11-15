//! Cover image downloads.

use std::time::Duration;

use anyhow::{Context, Result, bail};

use super::text::{normalize_url, snippet};
use super::{ApiClient, REFERER, UA};

impl ApiClient {
    pub async fn image_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let url = cover_thumbnail_url(url);
        if let Some(bytes) = self.cache.get_cover(&url) {
            return Ok(bytes);
        }

        let host = reqwest::Url::parse(&url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_string));
        let mut last_error = None;

        for attempt in 0..2 {
            match self.try_image_bytes(&url).await {
                Ok(bytes) => {
                    self.cache.put_cover(&url, &bytes);
                    return Ok(bytes);
                }
                Err(error) => {
                    last_error = Some(error);
                    if attempt == 0 {
                        if let Some(host) = &host {
                            self.resolver.forget(host).await;
                        }
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("下载封面失败")))
    }

    async fn try_image_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let response = self
            .http
            .get(url)
            .header("User-Agent", UA)
            .header("Referer", REFERER)
            .send()
            .await
            .context("下载封面失败")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("封面 HTTP {status}: {}", snippet(&body));
        }
        Ok(response.bytes().await?.to_vec())
    }
}
/// Ask Bilibili's image service for a small thumbnail instead of the original
/// 4K cover. The 32x16 terminal grid needs nothing larger.
pub(crate) fn cover_thumbnail_url(url: &str) -> String {
    let url = normalize_url(url);
    if !url.contains("hdslb.com") || url.contains('@') {
        return url;
    }
    if let Some((base, query)) = url.split_once('?') {
        format!("{base}@640w_360h.jpg?{query}")
    } else {
        format!("{url}@640w_360h.jpg")
    }
}
