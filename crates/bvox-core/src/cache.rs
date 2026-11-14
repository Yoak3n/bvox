//! On-disk cache for audio streams, covers, lyrics and video metadata.
//!
//! The frontend picks the root via `CorePaths` (XDG cache dir on desktop,
//! per-app cache on Android). Layout:
//!
//! ```text
//! audio/<bvid>_<cid>_<quality_id>_<bandwidth>.m4s
//! covers/<md5(thumbnail url)>.img
//! lyrics/<bvid>_<cid>.json
//! view/<bvid>.json
//! ```
//!
//! Audio is written to a `.part` file and renamed into place, so a truncated
//! download never looks like a cache hit. The whole cache is bounded by the
//! configured limit (default 1 GiB, changeable at runtime via
//! [`Cache::set_limit_mb`]); eviction drops the least recently used files.
//!
//! [`CacheMode`] picks what is stored at all: auto caches audio plus covers,
//! lyrics and metadata; small-files skips audio downloads and keeps the small
//! files; off bypasses the cache entirely. A size limit of 0 still disables
//! the cache as a whole for callers of the legacy limit knob.
//!
//! [`Cache::stats`] reports the space used per category and [`Cache::clear`]
//! empties the cache from the UI; in-flight `.part` downloads are left alone
//! so a running download cannot be corrupted by a cleanup.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

mod audio;
mod eviction;
mod media;
mod model;
mod paths;
#[cfg(test)]
mod tests;

pub use model::{CacheMode, CacheStats, CachedAudio, CachedTrack, format_bytes};

pub const CACHE_DIR_NAME: &str = "bvox";
pub const DEFAULT_LIMIT_MB: u64 = 1024;

pub struct Cache {
    root: PathBuf,
    /// Current cap in bytes, 0 when the cache is disabled. Atomic so the UI
    /// can change it while downloads run.
    limit_bytes: AtomicU64,
    /// What may be stored, changeable at runtime from the UI.
    mode: AtomicU8,
    /// `bvid:cid` keys with a background download in progress.
    inflight: Mutex<HashSet<String>>,
}

impl Cache {
    pub fn new(root: PathBuf, limit_mb: u64, mode: CacheMode) -> Self {
        let cache = Self {
            root,
            limit_bytes: AtomicU64::new(limit_mb.saturating_mul(1024 * 1024)),
            mode: AtomicU8::new(mode.as_u8()),
            inflight: Mutex::new(HashSet::new()),
        };
        cache.cleanup_stale_parts();
        cache
    }

    fn limit_bytes(&self) -> u64 {
        self.limit_bytes.load(Ordering::Relaxed)
    }

    pub fn mode(&self) -> CacheMode {
        CacheMode::from_u8(self.mode.load(Ordering::Relaxed))
    }

    /// Switch what the cache stores. Existing files are kept; the caller
    /// offers a separate clear action. A size limit of 0 still wins, but
    /// choosing a caching mode again restores the default cap so the choice
    /// is not silently inert.
    pub fn set_mode(&self, mode: CacheMode) {
        if mode != CacheMode::Off && self.limit_bytes() == 0 {
            self.limit_bytes.store(
                DEFAULT_LIMIT_MB.saturating_mul(1024 * 1024),
                Ordering::Relaxed,
            );
        }
        self.mode.store(mode.as_u8(), Ordering::Relaxed);
    }

    /// Audio is cached (and read back) only in auto mode.
    pub fn audio_enabled(&self) -> bool {
        self.mode() == CacheMode::Auto && self.limit_bytes() > 0
    }

    /// Covers, lyrics and metadata are cached in every mode except off.
    pub fn metadata_enabled(&self) -> bool {
        self.mode() != CacheMode::Off && self.limit_bytes() > 0
    }

    pub fn begin_download(&self, key: &str) -> bool {
        if !self.audio_enabled() {
            return false;
        }
        self.inflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(key.to_string())
    }

    pub fn end_download(&self, key: &str) {
        self.inflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(key);
    }
}
