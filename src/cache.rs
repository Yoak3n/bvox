//! On-disk cache for audio streams, covers, lyrics and video metadata.
//!
//! Layout under `$XDG_CACHE_HOME/bili-music` (override with `BILI_CACHE_DIR`):
//!
//! ```text
//! audio/<bvid>_<cid>_<quality_id>_<bandwidth>.m4s
//! covers/<md5(thumbnail url)>.img
//! lyrics/<bvid>_<cid>.json
//! view/<bvid>.json
//! ```
//!
//! Audio is written to a `.part` file and renamed into place, so a truncated
//! download never looks like a cache hit. The whole cache is bounded by
//! `BILI_CACHE_MB` (default 1 GiB); eviction drops the least recently used
//! files. `BILI_CACHE_MB=0` disables the cache entirely.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::api::{LyricLine, VideoDetail};

const CACHE_DIR_NAME: &str = "bili-music";
const DEFAULT_LIMIT_MB: u64 = 1024;
const VIEW_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const STALE_PART_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Debug)]
pub struct CachedAudio {
    pub path: PathBuf,
    pub quality_id: i64,
    pub bandwidth: i64,
}

#[derive(Serialize, Deserialize)]
struct CachedView {
    fetched_at: u64,
    detail: VideoDetail,
}

pub struct Cache {
    root: PathBuf,
    limit_bytes: u64,
    enabled: bool,
    /// `bvid:cid` keys with a background download in progress.
    inflight: Mutex<HashSet<String>>,
}

impl Cache {
    pub fn new() -> Self {
        let limit_bytes = cache_limit_bytes();
        let cache = Self {
            root: cache_root(),
            limit_bytes,
            enabled: limit_bytes > 0,
            inflight: Mutex::new(HashSet::new()),
        };
        cache.cleanup_stale_parts();
        cache
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Return a completed audio file for this page, preferring the newest copy.
    pub fn find_audio(&self, bvid: &str, cid: i64) -> Option<CachedAudio> {
        if !self.enabled {
            return None;
        }

        let entries = std::fs::read_dir(self.root.join("audio")).ok()?;
        let mut best: Option<(SystemTime, CachedAudio)> = None;

        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                continue;
            };
            let Some((quality_id, bandwidth)) = parse_audio_file_name(&name, bvid, cid) else {
                continue;
            };
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let candidate = CachedAudio {
                path: entry.path(),
                quality_id,
                bandwidth,
            };
            if best
                .as_ref()
                .is_none_or(|(best_modified, _)| modified >= *best_modified)
            {
                best = Some((modified, candidate));
            }
        }

        let (_, cached) = best?;
        touch(&cached.path);
        Some(cached)
    }

    pub fn audio_target(&self, bvid: &str, cid: i64, quality_id: i64, bandwidth: i64) -> PathBuf {
        self.root
            .join("audio")
            .join(audio_file_name(bvid, cid, quality_id, bandwidth))
    }

    pub fn audio_part(&self, final_path: &Path) -> PathBuf {
        let mut name = final_path
            .file_name()
            .map(ToOwned::to_owned)
            .unwrap_or_default();
        // Include the pid so two running instances cannot write the same part.
        name.push(format!(".{}.part", std::process::id()));
        final_path.with_file_name(name)
    }

    /// Atomically move a finished download into place, drop older encodings of
    /// the same page and enforce the cache size limit.
    pub fn commit_audio(&self, part: &Path, bvid: &str, cid: i64, final_path: &Path) -> Result<()> {
        std::fs::rename(part, final_path).context("提交音频缓存失败")?;
        self.remove_other_audio(bvid, cid, final_path);
        self.evict(Some(final_path));
        Ok(())
    }

    pub fn get_cover(&self, url: &str) -> Option<Vec<u8>> {
        if !self.enabled {
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
        if !self.enabled {
            return;
        }
        let path = self.cover_path(url);
        if write_atomic(&path, bytes).is_ok() {
            self.evict(Some(&path));
        }
    }

    pub fn get_lyrics(&self, bvid: &str, cid: i64) -> Option<Vec<LyricLine>> {
        if !self.enabled {
            return None;
        }
        let path = self.lyrics_path(bvid, cid);
        let bytes = std::fs::read(&path).ok()?;
        let lines = serde_json::from_slice::<Vec<LyricLine>>(&bytes).ok()?;
        if lines.is_empty() {
            return None;
        }
        touch(&path);
        Some(lines)
    }

    /// Empty lyric lists are intentionally not stored: subtitles often become
    /// available only after login, and a cached empty result would hide them.
    pub fn put_lyrics(&self, bvid: &str, cid: i64, lines: &[LyricLine]) {
        if !self.enabled || lines.is_empty() {
            return;
        }
        let path = self.lyrics_path(bvid, cid);
        if let Ok(bytes) = serde_json::to_vec(lines)
            && write_atomic(&path, &bytes).is_ok()
        {
            self.evict(Some(&path));
        }
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
        if !self.enabled {
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

    /// Claim a background download for `bvid:cid`. Returns false when caching is
    /// disabled or a download for the same page is already running.
    pub fn begin_download(&self, key: &str) -> bool {
        if !self.enabled {
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

    fn load_view(&self, bvid: &str, max_age: Option<Duration>) -> Option<VideoDetail> {
        if !self.enabled {
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

    fn cover_path(&self, url: &str) -> PathBuf {
        let digest = format!("{:x}", md5::compute(url.as_bytes()));
        self.root.join("covers").join(format!("{digest}.img"))
    }

    fn lyrics_path(&self, bvid: &str, cid: i64) -> PathBuf {
        self.root.join("lyrics").join(format!("{bvid}_{cid}.json"))
    }

    fn view_path(&self, bvid: &str) -> PathBuf {
        self.root.join("view").join(format!("{bvid}.json"))
    }

    fn remove_other_audio(&self, bvid: &str, cid: i64, keep: &Path) {
        let Ok(entries) = std::fs::read_dir(self.root.join("audio")) else {
            return;
        };
        let prefix = format!("{bvid}_{cid}_");
        for entry in entries.flatten() {
            let path = entry.path();
            if path == keep {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name.starts_with(&prefix) && name.ends_with(".m4s") {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    /// Drop least recently used files until the cache is back under 90% of the
    /// configured limit. `keep` is the file that was just written.
    fn evict(&self, keep: Option<&Path>) {
        if !self.enabled {
            return;
        }

        let mut files = Vec::new();
        collect_files(&self.root, &mut files);
        let mut total = files.iter().map(|file| file.size).sum::<u64>();
        if total <= self.limit_bytes {
            return;
        }

        files.sort_by_key(|file| file.modified);
        let target = self.limit_bytes - self.limit_bytes / 10;
        for file in files {
            if total <= target {
                break;
            }
            // Parts belong to in-flight downloads; leave them alone.
            if file
                .path
                .extension()
                .and_then(|extension| extension.to_str())
                == Some("part")
            {
                continue;
            }
            if keep.is_some_and(|keep| keep == file.path) {
                continue;
            }
            if std::fs::remove_file(&file.path).is_ok() {
                total = total.saturating_sub(file.size);
            }
        }
    }

    fn cleanup_stale_parts(&self) {
        if !self.enabled {
            return;
        }
        let Ok(entries) = std::fs::read_dir(self.root.join("audio")) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("part") {
                continue;
            }
            let stale = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
                .is_some_and(|modified| modified.elapsed().is_ok_and(|age| age > STALE_PART_AGE));
            if stale {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

struct CacheFile {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

fn collect_files(dir: &Path, out: &mut Vec<CacheFile>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_files(&entry.path(), out);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        out.push(CacheFile {
            path: entry.path(),
            size: metadata.len(),
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        });
    }
}

fn audio_file_name(bvid: &str, cid: i64, quality_id: i64, bandwidth: i64) -> String {
    format!("{bvid}_{cid}_{quality_id}_{bandwidth}.m4s")
}

fn parse_audio_file_name(name: &str, bvid: &str, cid: i64) -> Option<(i64, i64)> {
    let rest = name
        .strip_prefix(&format!("{bvid}_{cid}_"))?
        .strip_suffix(".m4s")?;
    let mut parts = rest.split('_');
    let quality_id = parts.next()?.parse().ok()?;
    let bandwidth = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((quality_id, bandwidth))
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn touch(path: &Path) {
    if let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("缓存路径缺少父目录")?;
    std::fs::create_dir_all(parent).context("创建缓存目录失败")?;
    let suffix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let tmp = path.with_extension(format!("tmp.{}.{suffix}", std::process::id()));
    std::fs::write(&tmp, bytes).context("写入缓存临时文件失败")?;
    std::fs::rename(&tmp, path).context("提交缓存文件失败")
}

fn cache_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("BILI_CACHE_DIR")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
    {
        return dir;
    }

    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join(CACHE_DIR_NAME)
}

fn cache_limit_bytes() -> u64 {
    let mb = std::env::var("BILI_CACHE_MB")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_LIMIT_MB);
    mb.saturating_mul(1024 * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_file_names_roundtrip() {
        let name = audio_file_name("BV1xx411c7mD", 12345, 30280, 204800);
        assert_eq!(
            parse_audio_file_name(&name, "BV1xx411c7mD", 12345),
            Some((30280, 204800))
        );
    }

    #[test]
    fn malformed_audio_file_names_are_rejected() {
        assert_eq!(
            parse_audio_file_name("BV1xx411c7mD_1_30280_204800.m4s.part", "BV1xx411c7mD", 1),
            None
        );
        assert_eq!(
            parse_audio_file_name("BV1xx411c7mD_2_30280_204800.m4s", "BV1xx411c7mD", 1),
            None
        );
        assert_eq!(
            parse_audio_file_name("BV1xx411c7mD_1_30280_x.m4s", "BV1xx411c7mD", 1),
            None
        );
    }

    fn test_cache() -> (Cache, PathBuf) {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "bili-music-cache-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let cache = Cache {
            root: root.clone(),
            limit_bytes: 1024 * 1024,
            enabled: true,
            inflight: Mutex::new(HashSet::new()),
        };
        (cache, root)
    }

    #[test]
    fn audio_commit_and_lookup() {
        let (cache, root) = test_cache();
        let target = cache.audio_target("BV1xx411c7mD", 7, 30280, 204800);
        let part = cache.audio_part(&target);
        std::fs::create_dir_all(part.parent().unwrap()).unwrap();
        std::fs::write(&part, b"m4s-bytes").unwrap();

        cache
            .commit_audio(&part, "BV1xx411c7mD", 7, &target)
            .unwrap();

        let cached = cache.find_audio("BV1xx411c7mD", 7).unwrap();
        assert_eq!(cached.quality_id, 30280);
        assert_eq!(cached.bandwidth, 204800);
        assert_eq!(std::fs::read(&cached.path).unwrap(), b"m4s-bytes");
        assert!(!part.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn lyrics_and_cover_roundtrip() {
        let (cache, root) = test_cache();
        let lines = vec![LyricLine {
            from: 1.5,
            to: 3.0,
            text: "第一句".to_string(),
        }];
        cache.put_lyrics("BV1xx411c7mD", 42, &lines);
        let loaded = cache.get_lyrics("BV1xx411c7mD", 42).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text, "第一句");

        // Empty lyrics are deliberately not cached.
        cache.put_lyrics("BV1xx411c7mD", 43, &[]);
        assert!(cache.get_lyrics("BV1xx411c7mD", 43).is_none());

        let cover_url = "https://i0.hdslb.com/a.jpg@640w_360h.jpg";
        cache.put_cover(cover_url, b"image-bytes");
        assert_eq!(
            cache.get_cover(cover_url).as_deref(),
            Some(b"image-bytes".as_slice())
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn view_cache_respects_ttl_with_stale_fallback() {
        let (cache, root) = test_cache();
        let detail = VideoDetail {
            bvid: "BV1xx411c7mD".to_string(),
            aid: 1,
            cid: 42,
            title: "标题".to_string(),
            author: "UP".to_string(),
            duration: 60,
            pages: vec![crate::api::VideoPage {
                cid: 42,
                duration: 60,
                part: "P1".to_string(),
            }],
            pic: None,
        };
        cache.put_view("BV1xx411c7mD", &detail);

        let fresh = cache.get_view("BV1xx411c7mD").unwrap();
        assert_eq!(fresh.cid, 42);
        assert_eq!(fresh.title, "标题");

        // Age the entry past the 24h TTL.
        let path = root.join("view/BV1xx411c7mD.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["fetched_at"] = serde_json::json!(0);
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        assert!(cache.get_view("BV1xx411c7mD").is_none());
        assert!(cache.get_stale_view("BV1xx411c7mD").is_some());
        let _ = std::fs::remove_dir_all(root);
    }
}
