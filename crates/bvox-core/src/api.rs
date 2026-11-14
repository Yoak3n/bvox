//! Bilibili API: WBI search, QR login, favorites, subtitles, covers and
//! audio-only DASH streams.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::sync::Mutex;

use crate::CorePaths;
use crate::cache::Cache;
use login::load_saved_cookie;
use transport::Ipv4FirstResolver;

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
pub const REFERER: &str = "https://www.bilibili.com/";

pub const CONFIG_DIR_NAME: &str = "bvox";

mod account;
mod audio;
mod image;
#[cfg(test)]
mod library_tests;
mod login;
mod lyrics;
mod model;
mod playlists;
mod quality;
mod search;
mod sign;
#[cfg(test)]
mod streaming_tests;
#[cfg(test)]
mod tests;
mod text;
mod transport;
mod video;

pub(crate) use image::cover_thumbnail_url;
pub use model::{
    AudioSource, AudioStream, FavoriteFolder, LyricLine, QrLogin, QrPhase, SearchPage, UserInfo,
    VideoDetail, VideoItem, VideoPage,
};
pub use quality::{AudioQuality, PlayMode, quality_label};
pub(crate) use text::clean_title;

pub struct ApiClient {
    paths: CorePaths,
    http: reqwest::Client,
    resolver: Arc<Ipv4FirstResolver>,
    cache: Cache,
    keys: Mutex<Option<(String, String, Instant)>>,
    buvid: Mutex<Option<String>>,
    cookie: Mutex<Option<String>>,
    /// Serializes read-modify-write of the per-account hidden playlist file.
    hidden_playlists_lock: Mutex<()>,
}

impl ApiClient {
    pub fn new(paths: CorePaths) -> Result<Self> {
        let cookie = std::env::var("BVOX_COOKIE")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| load_saved_cookie(&paths.data_dir));

        let resolver = Arc::new(Ipv4FirstResolver::new());
        let http = reqwest::Client::builder()
            .dns_resolver(resolver.clone())
            .timeout(Duration::from_secs(20))
            .build()
            .context("创建 HTTP 客户端失败")?;

        let cache_mb = paths.cache_mb;
        let cache_mode = paths.cache_mode;
        let cache_dir = paths.cache_dir.clone();
        Ok(Self {
            http,
            resolver,
            paths,
            cache: Cache::new(cache_dir, cache_mb, cache_mode),
            keys: Mutex::new(None),
            buvid: Mutex::new(None),
            cookie: Mutex::new(cookie.clone()),
            hidden_playlists_lock: Mutex::new(()),
        })
    }

    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    /// Directory holding the cookie and the small per-user preference files.
    pub fn data_dir(&self) -> &std::path::Path {
        &self.paths.data_dir
    }

    pub fn audio_cache_enabled(&self) -> bool {
        self.cache.audio_enabled()
    }
}

pub fn format_secs(total: i64) -> String {
    let total = total.max(0);
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
