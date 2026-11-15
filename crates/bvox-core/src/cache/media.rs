//! Small-file cache: covers, lyrics and video metadata.

use std::time::Duration;

use super::Cache;
use super::model::{CachedLyrics, CachedView, LYRICS_VERSION, VIEW_TTL};
use super::paths::{touch, unix_secs, write_atomic};
use crate::model::{LyricLine, VideoDetail};

impl Cache {
    pub fn get_cover(&self, url: &str) -> Option<Vec<u8>> {
        if !self.metadata_enabled() {
            return None;
        }
        let path = self.cover_path(url);
        let bytes = std::fs::read(&path).ok()?;
        if bytes.is_empty() {
            return None;
        }
        touch(&path);
        Some(bytes)
    }

    pub fn put_cover(&self, url: &str, bytes: &[u8]) {
        if !self.metadata_enabled() {
            return;
        }
        let path = self.cover_path(url);
        if write_atomic(&path, bytes).is_ok() {
            self.evict(Some(&path));
        }
    }

    pub fn get_lyrics(&self, bvid: &str, cid: i64) -> Option<Vec<LyricLine>> {
        if !self.metadata_enabled() {
            return None;
        }
        let path = self.lyrics_path(bvid, cid);
        let bytes = std::fs::read(&path).ok()?;
        let cached = serde_json::from_slice::<CachedLyrics>(&bytes).ok()?;
        if cached.version != LYRICS_VERSION || cached.lines.is_empty() {
            return None;
        }
        touch(&path);
        Some(cached.lines)
    }

    /// Empty lyric lists are intentionally not stored: subtitles often become
    /// available only after login, and a cached empty result would hide them.
    pub fn put_lyrics(&self, bvid: &str, cid: i64, lines: &[LyricLine]) {
        if !self.metadata_enabled() || lines.is_empty() {
            return;
        }
        let path = self.lyrics_path(bvid, cid);
        let cached = CachedLyrics {
            version: LYRICS_VERSION,
            lines: lines.to_vec(),
        };
        if let Ok(bytes) = serde_json::to_vec(&cached)
            && write_atomic(&path, &bytes).is_ok()
        {
            self.evict(Some(&path));
        }
    }

    /// Drop cached lyrics for a page. Used by the manual "re-fetch lyrics"
    /// action, which must bypass the cache.
    pub fn remove_lyrics(&self, bvid: &str, cid: i64) {
        if !self.metadata_enabled() {
            return;
        }
        let _ = std::fs::remove_file(self.lyrics_path(bvid, cid));
    }

    /// Metadata at most a day old. Returns `None` when the entry is missing or
    /// stale.
    pub fn get_view(&self, bvid: &str) -> Option<VideoDetail> {
        self.load_view(bvid, Some(VIEW_TTL))
    }

    /// Metadata of any age; used as an offline/risk-control fallback.
    pub fn get_stale_view(&self, bvid: &str) -> Option<VideoDetail> {
        self.load_view(bvid, None)
    }

    pub fn put_view(&self, bvid: &str, detail: &VideoDetail) {
        if !self.metadata_enabled() {
            return;
        }
        let path = self.view_path(bvid);
        let cached = CachedView {
            fetched_at: unix_secs(),
            detail: detail.clone(),
        };
        if let Ok(bytes) = serde_json::to_vec(&cached)
            && write_atomic(&path, &bytes).is_ok()
        {
            self.evict(Some(&path));
        }
    }

    fn load_view(&self, bvid: &str, max_age: Option<Duration>) -> Option<VideoDetail> {
        if !self.metadata_enabled() {
            return None;
        }
        let path = self.view_path(bvid);
        let bytes = std::fs::read(&path).ok()?;
        let cached = serde_json::from_slice::<CachedView>(&bytes).ok()?;
        if let Some(max_age) = max_age
            && unix_secs().saturating_sub(cached.fetched_at) > max_age.as_secs()
        {
            return None;
        }
        touch(&path);
        Some(cached.detail)
    }
}
